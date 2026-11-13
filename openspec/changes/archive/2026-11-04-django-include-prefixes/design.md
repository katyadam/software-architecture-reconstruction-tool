## Context

See proposal.md - Why. The resolver keys routers by `(file, scope, variable)`. Every `Attach` edge has its parent and child in one file; only function-call arguments cross files. Django's child is another module's `urlpatterns`, named by a dotted string. All Django endpoints already carry `router_variable = "urlpatterns"` at module scope, so the child side needs no extractor change.

Python call arguments are split naively on `,` and `=` (`calls/extractor.rs`). So `include(("a.b", "app"), namespace="x")` reaches the adapter as fragments, and `path("id=<x>/", ...)` as a keyword argument. The adapter works on those fragments rather than fixing the shared split, which about 20 code sites read.

## Goals / Non-Goals

**Goals:**
- `include()` by module string, in the forms the specs list.
- Resolver stays free of Python and Django knowledge.

**Non-Goals:**
- `include(extra_patterns)` (list variable in the same file) and inline `include([path(...)])`. Their inner routes keep today's URI.
- DRF `include(router.urls)`: router-registered routes are not extracted at all.
- `include(module)` with an imported module object.
- `ROOT_URLCONF`: a module that nothing includes is served as declared.
- Normalizing a `re_path` child's own URI: `^x/$` stays, so the result is `/api/^x/$`.
- Legacy `url()`: the endpoint strategy ignores it too.
- An `include()` nested inside an inline or list-variable include gets only its own prefix: `/v1/x/`, true `/api/v1/x/`.
- Converter syntax held in a constant prefix stays as written: `/<int:org>/x/`.

## Decisions

### D1. New fact variant `AttachModule`
```rust
/// `child` of the first module found at `paths` is served under `parent` + `prefix`.
/// `paths` are relative to the fact's directory or an ancestor, in preference order.
AttachModule { scope: Scope, parent: String, paths: Vec<String>, child: String, prefix: String }
```
- Alternative: overload `Attach.child` with a path. Rejected: every `Attach` reader would have to tell the two apart.
- Alternative: expand `include()` during extraction. Rejected: extraction sees one file at a time.
- Alternative: resolve dotted names in the resolver. Rejected: that is Python knowledge, against the neutral IR.

### D2. Finding the module
Walk up from the including file's directory, deepest first. At each level try each entry of `paths` in `by_path`; the first hit wins. This mirrors Python: a module root is an ancestor of the including file. So a same-named module in a sibling service never matches, without any service concept. Cost is depth times two map lookups per fact.
- Alternative: suffix match plus most shared path segments, as `callee_files` does. Rejected: it would attach `svc_b/accounts/urls.py` for a third-party `accounts.urls` in `svc_a`.
- Alternative: import graph. Rejected: a string argument is not an import.

The walk reaches the corpus root, so a root-level `accounts/urls.py` can match a third-party name in any service. Accepted as rare.

The including file never matches itself: that module would import itself, which Django rejects, so the walk continues upward. A dotted name qualified with a sibling service directory (`svc_b.accounts.urls`) does resolve there, by the same ancestor rule. Accepted: such a name is what the source says.

### D3. Resolver edges carry the parent's file
`attach` values gain the parent's file index and scope. A local `Attach` stores its own file; `AttachModule` is stored under `(found file, "", child)` with the including file as parent. `PrefixExpr.file_path` becomes the parent's file, which equals today's value for local edges. So a constant prefix evaluates in the file that wrote it, and pass 3 needs no change. Existing cycle and depth guards apply, since `VarKey` already includes the file.

### D4. Django adapter `route_facts/django.rs`
A custom function, not an idiom table, since the child is a string inside a nested call. For each module-level `path` / `re_path` call:
- Positional arguments 0 and 1 must exist. A keyword-shaped argument 0, such as a route holding `=`, yields no fact.
- Argument 1 trimmed: strip `include(`, then any `(`, then any trailing `)`. The rest must be a quoted literal of dot-separated identifiers, else no fact. This handles the tuple and `namespace=` fragments.
- `a.b` -> `paths: ["a/b.py", "a/b/__init__.py"]`, `child` and `parent` are `urlpatterns`.
- `path` prefix: a quoted or `r`-quoted literal goes through `normalize_python_route`, then is re-quoted with `"`. Anything else passes as raw text for pass 3, which rejects what it cannot evaluate.
- `re_path` prefix: literals only. Strip a leading `^`. Any remaining regex metacharacter -> no fact.

The fact does not gate on a Django import, the same as the endpoint strategy.

### D5. Walk budget
`prefixes` costs fan-in^`MAX_DEPTH` per endpoint, and `MAX_CHAINS` only trims results after the walk. Cross-file includes make wide layers easy to write. Each endpoint gets `MAX_STEPS` calls of `prefixes`; past that the walk stops, with one warning, and keeps the chains found. `MAX_STEPS` is 100_000: the existing chain-cap test, fan-in 4 over 8 levels, already takes 21_845 calls, so a lower budget would cut walks that `MAX_CHAINS` handles today.
- Alternative: memoize per router. Rejected: cycle-guard results depend on the path.

## Risks / Trade-offs

- [A prefix holding `,`, a comment between arguments, `u""` or implicit concatenation, keyword `route=` / `view=` / `arg=`, qualified `urls.include`] -> No fact, so today's URI. Listed in known limits.
- [Non-Django `path(x, include("y"))`] -> Only matters if `y` resolves in the project and the file has `urlpatterns` routes. Accepted.
- [Mixed URI shapes: prefixed routes start with `/`, unprefixed Django routes do not] -> Existing `join` behavior; matching strips the leading `/` (`synthesizer/src/sdg/matching.rs:149`).
- [No corpus exercises this] -> Tests are the only coverage, so each spec scenario gets a test.
