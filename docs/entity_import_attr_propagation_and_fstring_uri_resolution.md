# Entity-Import Attr Propagation and F-string URI Resolution

Two fixes added in this pipeline run extend URI resolution to cover patterns that
previously produced unresolved template strings in REST call output. They work
independently but compose in the common case where both apply in the same call chain.

See also:
- `cross_file_constant_resolution.md` — `pass_module` / `pass_attr` pipeline stages
- `captured_scopes_closure_evaluation.md` — outer-scope variable capture for nested functions

---

## Fix 1: Entity-Import Attr Propagation in `build_file_env`

**File**: `extractor-runtime/src/pipeline/pass3/restcalls.rs`

### Problem

`pass_attr::resolve_all` walks every file, finds class instantiation sites
(`settings = Settings()`), and emits per-file attribute defaults:

```
per_file_attrs["singletons.py"]["settings.as_url"] = "aaaa"
```

`build_file_env` (in Pass 3) injects those attrs into the eval env when the
_same_ file is being evaluated. But `jobs.py` does:

```python
from singletons import settings
```

When evaluating `jobs.py`, `build_file_env` needed to reach across the import
boundary and inject the `settings.*` attrs from `singletons.py` into `jobs.py`'s
env. The original code only propagated `ImportKind::Constant` imports that resolved
to a scalar string value via `per_file_module_consts`. That path handles
`from singletons import aaa_url` (a plain string), but not
`from singletons import settings` (an object instance).

Object instances like `settings = Settings()` are indexed by `FileDefinitionsIndex`
as `ImportKind::Constant` (global assignments), not as `ImportKind::Entity` (class
definitions). An earlier version of the fix used an `ImportKind::Entity` filter, which
made the new loop unreachable — the reviewer identified this as a critical bug.

### Fix

A second loop in `build_file_env` walks `file.imports`, resolves each import via
`project_ir.import_graph.lookup`, skips non-`Constant` imports, then looks up the
**source file** in `per_file_attrs` and injects all keys whose prefix matches
`"<codeword>."`:

```rust
let prefix = format!("{}.", import.codeword);
for (key, value) in source_attrs {
    if key.starts_with(&prefix) {
        env.entry(key.clone())
            .or_insert_with(|| (Some("String".to_string()), Expr::Literal(value.clone())));
    }
}
```

`or_insert_with` preserves the standard priority chain — project constants and
CLI overrides inserted earlier in `build_file_env` are never downgraded.

### Key-space contract

`per_file_attrs` has two levels of keys:

| Level | Key | Meaning |
|---|---|---|
| Outer | file path string | The **binding-site** file — where `x = ClassName()` appears |
| Inner | `"var_name.field"` | The instance variable name and the class field name |

The binding-site file is `singletons.py` (where `settings = Settings()`), not
`settings.py` (where the class is defined). The lookup must use
`resolved.source_file` from the import graph, which points to the file that defines
the imported name — here, `singletons.py`. The dot prefix (`"settings."`) prevents
a codeword `"s"` from spuriously matching keys like `"settings.x"`.

### What the symbolic evaluator does with the injected keys

The symbolic evaluator handles `Expr::Attr { object: Var("settings"), field: "as_url" }`
by constructing the dot-key `"settings.as_url"` and looking it up in the env. Once
the key is present (injected by the new loop), the attribute access resolves to
`Expr::Literal("aaaa")`.

---

## Fix 2: F-string URI Template Resolution in `get_resolved_parts`

**File**: `python-extractor/src/extraction/restcalls/evaluation/uri_generator.rs`

### Two URI patterns

**Pattern A — f-string assigned to a local variable** (worked before this fix):

```python
url = f"{base_url}/v1/jobs/{job_id}/lock/annotations/{annotation_id}"
http_client.put(url)
```

`identify_target_uri` extracts `"url"` as the template. `get_resolved_parts`
splits by `+`, looks up `"url"` in the env, finds `Expr::Literal(s)` where `s`
contains `{`, and calls `resolve_fstring(s, ...)`. This path was already working.

**Pattern B — inline f-string** (broken before this fix):

```python
http_client.put(f"{base_url}/v1/jobs/{job_id}/lock/annotations/{annotation_id}")
```

`identify_target_uri` strips the `f"..."` wrapper via `clean_python_string`,
leaving the content `{base_url}/v1/jobs/{job_id}/lock/annotations/{annotation_id}`
as the template. `get_resolved_parts` splits by `+` and tries to look up the entire
content as a variable name in `final_env`. That lookup misses. The old fallback
returned the template string unchanged, leaving the URI unresolved.

### Fix

A new wildcard arm in `get_resolved_parts` handles the miss case when the part
contains `{`:

```rust
_ if part.contains('{') => {
    all_parts.extend(resolve_fstring(part, analysis_result, enums_map));
}
```

`resolve_fstring` already handled all placeholder resolution logic. The new arm
routes the inline f-string content through the same path, so `{base_url}` is
looked up in the analysis result env and replaced with `"aaaa"` (or whatever the
env holds), while `{job_id}` (unresolved path parameter) is kept as `{job_id}`.

Both patterns now produce correct URIs. Pattern A continues to work via the
`Expr::Literal(s)` branch. Pattern B works via the new wildcard arm.

---

## How They Compose: Full Example Trace

The fixture in `cli/tests/fixtures/cross-file/` demonstrates both fixes working
together. The chain has four steps:

```
settings.py          singletons.py         jobs.py
-----------          -------------         -------
class Settings:      from settings         from singletons import settings
    as_url: str =      import Settings
        "aaaa"       settings = Settings()
                                           def add_routes_jobs(app):
                                             base_url = settings.as_url.rstrip("/")
                                             async def _(...):
                                               http_client.put(f"{base_url}/v1/...")
```

### Step 1 — `pass_attr` emits attribute defaults

`pass_attr::resolve_all` finds `settings = Settings()` in `singletons.py`, resolves
`Settings` to the class in `settings.py`, and emits:

```
per_file_attrs["singletons.py"]["settings.as_url"] = "aaaa"
```

The outer key is the binding-site file (`singletons.py`), not the class-definition
file (`settings.py`).

### Step 2 — `build_file_env` propagates across the import boundary

When building the env for `jobs.py`, the new loop in `build_file_env` sees:

```
import { codeword: "settings", source_file: "singletons.py", kind: Constant }
```

It fetches `per_file_attrs["singletons.py"]`, finds `"settings.as_url"`, and inserts
it into `jobs.py`'s env:

```
env["settings.as_url"] = Expr::Literal("aaaa")
```

### Step 3 — captured scopes propagate `base_url` to the inner closure

`build_captured_scopes` evaluates the outer callable `add_routes_jobs`. The
symbolic evaluator processes `base_url = settings.as_url.rstrip("/")`:

- `settings.as_url` resolves from the env -> `Expr::Literal("aaaa")`
- `.rstrip("/")` acts as identity on a string literal (known statix behavior)
- Result: `base_url = "aaaa"`

This is stored in `captured_scopes[inner_function_hash]["base_url"]`.

### Step 4 — f-string URI resolved

The inner closure's REST call has `target_uri = "{base_url}/v1/jobs/{job_id}/lock/annotations/{annotation_id}"` (after `clean_python_string` strips the `f"..."` wrapper).

`get_resolved_parts` receives this template. The env lookup on the full string misses,
but `part.contains('{')` is true, so `resolve_fstring` is called. It resolves
`{base_url}` to `"aaaa"` from the env, leaves `{job_id}` and `{annotation_id}` as
path-parameter placeholders, and produces:

```
"aaaa/v1/jobs/{job_id}/lock/annotations/{annotation_id}"
```

---

## Relevant Files

| File | Role |
|---|---|
| `extractor-runtime/src/pipeline/pass3/restcalls.rs` | `build_file_env` with the new attr-propagation loop |
| `extractor-runtime/src/pipeline/pass_attr.rs` | Emits `per_file_attrs` keyed by binding-site file |
| `python-extractor/src/extraction/restcalls/evaluation/uri_generator.rs` | `get_resolved_parts` fallback arm for inline f-strings |
| `python-extractor/src/extraction/restcalls/identification/method_call.rs` | `identify_target_uri` / `clean_python_string` — strips `f"..."` wrapper |
| `cli/tests/fixtures/cross-file/` | Fixture demonstrating the full three-file chain |
| `cli/tests/e2e/scenario_cross_file.rs` | E2E test asserting URIs resolve to `"aaaa"` |
