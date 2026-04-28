# Cross-File Constant Resolution — Module-Level Assignment Propagation

## Problem

Python microservices commonly derive module-level URL variables from class field
defaults and then import them into consumer files:

```python
# settings.py
class Settings:
    aaa_service_url: str = "http://aaa/"

# singletons.py
from settings import Settings
settings = Settings()
aaa_url = settings.aaa_service_url.rstrip("/")

# api/aaa_connector.py
from singletons import aaa_url

async def fetch(app_id):
    return await http_client.get(f"{aaa_url}/v1/customer/apps/{app_id}")
```

Three things prevented URI resolution before this pass:

1. `aaa_url = settings.aaa_service_url.rstrip("/")` is a module-level statement,
   not inside a function body. The symbolic evaluator only runs on callables, so
   `aaa_url` was never computed.
2. `pass2::collect_constants` only captures `UPPER_SNAKE_CASE` names as project
   constants; lowercase `aaa_url` was ignored.
3. Even if the name had been captured, the stored value would be the raw source
   text `"settings.aaa_service_url.rstrip(\"/\")"` — not an evaluated literal.

Result: the REST call in `aaa_connector.py` saw `aaa_url` as an unresolved `Var`
and the URI stayed as the raw f-string template.

## Solution Overview

Two new pipeline stages sit between `pass_attr` and Pass 3:

```
pass_attr::resolve_all   -> PerFileAttrMap
pass_module::resolve_all -> PerFileModuleConsts   (Stage A)
pass3::evaluate                                   (Stage B wired inside)
```

**Stage A** evaluates each Python file's synthetic `<module>()` callable and
harvests every variable that reduces to a literal. **Stage B** (inside Pass 3's
restcalls evaluator) walks the consumer file's imports and injects those literals
into the file's evaluation env.

## Stage A — Synthetic `<module>()` callable

### A.1 Synthesis at extraction time

`python-extractor/src/extraction/module/` builds a `ParsedCallable` whose AST is
the sequence of top-level statements of the file. It is appended to `callables`
in `extract_syntactic` after `CallablesExtractor` runs.

- `metadata.name = "<module>()"` — angle brackets prevent collision with any real
  Python identifier.
- `ast.statements` — all top-level `expression_statement` nodes (assignments,
  augmented assignments, simple expressions).
- `ast.nested = []`

Java files have no such synthetic callable; `pass_module` skips them silently.

### A.2 Per-file evaluation (`pass_module::resolve_all`)

For each file in `project_ir.files`:

1. Build an initial env from project constants (Pass 2), then layer CLI
   `external_constants` (fill-gaps only), then layer `per_file_attrs` (lowest
   priority among explicit sources).
2. Locate the `<module>()` entry in the file's callable map. If absent (Java, or
   a file with no top-level statements), skip.
3. Evaluate the module body **one statement at a time**. Each iteration swaps the
   `<module>()` entry in the callables map to a single-statement AST and calls
   `symbolic_evaluation_with_env`. On success the accumulated env is updated; on
   error (e.g. an unresolvable `Var` on the RHS) that statement is skipped and
   the env is carried forward unchanged. This prevents one unresolvable
   assignment (e.g. `logger = logging.getLogger(...)`) from discarding all
   previously computed literals.
4. Walk the final accumulated env; collect every `(name, Expr::Literal(value))`
   pair into a `HashMap<String, String>`.

The result is `PerFileModuleConsts`: `HashMap<file_path, HashMap<var_name, literal>>`.

**Known behavior:** `rstrip("/")` currently acts as identity — the symbolic
evaluator returns the receiver unchanged for unknown method calls on a string
(`statix/src/symbolic.rs::visit_call`). URIs therefore keep any trailing slash
present in the class field default.

**UPPER_SNAKE_CASE clash:** `pass2::collect_constants` captures `UPPER_SNAKE_CASE`
module globals as raw source-text project constants and places them in the base
env before `pass_module` runs. If such a constant's raw text is also a valid
literal (e.g. `BASE = "/api"`), the Pass 2 value wins via `or_insert_with`.
Use lowercase names for module-level derived globals to avoid this.

## Stage B — Import-graph propagation (inside Pass 3)

`build_file_env` (extracted from `evaluate_file_restcalls` in
`pass3/restcalls.rs`) layers the resolved module consts into the file-level env
in three steps:

1. **Same-file own consts** — functions reference their own file's module globals
   directly (no import statement). Pulled from `per_file_module_consts[file_path]`.
2. **Cross-file imports** — for each `import` in `file.imports`:
   - Look up `(file_path, import.codeword)` via `ImportGraph::lookup`.
   - Skip unless `resolved.kind == ImportKind::Constant`.
   - Look up `per_file_module_consts[resolved.source_file][resolved.fully_qualified_name]`.
   - Insert under `import.codeword` (the local name) so `from m import x as y`
     binds the literal to `y` in the consumer's env.
3. All insertions use `or_insert_with`, preserving the priority chain:
   project constants > CLI externals > attr bucket > own module consts > imported
   module consts.

## Priority chain (highest to lowest)

| Source | Where set |
|---|---|
| Project constants (UPPER_SNAKE_CASE, Pass 2) | `build_constants_env` |
| CLI `external_constants` | `build_constants_env` |
| Per-file attribute defaults (`pass_attr`) | `build_file_env` |
| Same-file module-level literals (`pass_module`) | `build_file_env` |
| Imported module-level literals (`pass_module` via import graph) | `build_file_env` |

## Data flow summary

```
python-extractor::extract_syntactic
  -> build_module_callable (top-level stmts -> ParsedCallable "<module>()")
  -> appended to FileRecord.callables

pass_attr::resolve_all
  -> PerFileAttrMap: file -> { "settings.x" -> "http://aaa/" }

pass_module::resolve_all
  for each Python file:
    initial_env = project_consts + external_consts + per_file_attrs
    for each top-level stmt:
      eval single stmt with acc_env -> update acc_env (skip on Err)
    collect Expr::Literal entries -> PerFileModuleConsts[file]

pass3::evaluate (restcalls path)
  build_file_env(file):
    env = constants_env
        + per_file_attrs[file]           (or_insert_with)
        + per_file_module_consts[file]   (or_insert_with, own consts)
        + per_file_module_consts[source] (or_insert_with, per import)
  evaluate_single_restcall(env) -> resolved URI
```

## Relevant files

| File | Role |
|---|---|
| `python-extractor/src/extraction/module/` | Synthesizes the `<module>()` callable |
| `python-extractor/src/extraction/parse.rs` | Appends it to `parsed_callables` |
| `extractor-runtime/src/pipeline/pass_module.rs` | Stage A: per-file module body evaluation |
| `extractor-runtime/src/pipeline/pass3/restcalls.rs` | Stage B: `build_file_env`, `evaluate_file_restcalls` |
| `extractor-runtime/src/pipeline/pass3/mod.rs` | Threads `PerFileModuleConsts` into `evaluate`; filters `<module>()` from `EvaluatedIR.callables` |
| `cli/src/main.rs`, `extractor-runtime/src/api/service.rs` | Call sites: `pass_module::resolve_all` |
| `extractor-runtime/tests/pipeline/module_resolution.rs` | 10 integration tests covering all scenarios |
