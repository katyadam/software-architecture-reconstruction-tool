## Context

See proposal.md (Why). Pass 2 (`extractor-runtime/src/pipeline/pass2/mod.rs`, `build_project_ir`) holds all files of all languages as `TypedFileRecord`s plus the project `ImportGraph`. Language-neutral data already there:

- `endpoints`: `Endpoint { uri, http_method, function_name, file_path, router_variable, .. }`. `router_variable` is the object a route is registered on (`app` in `@app.get`); Python sets it, Java/Go do not. No enclosing function.
- `call_statements`: `CallStatement { function_name /* full callee text, e.g. "app.mount" */, arguments: [Argument { assigned_variable /* keyword or "" */, value }], enclosing_function_name, .. }`. Python arguments come from a naive `split(',')` / `split('=')` (`python-extractor/src/extraction/calls/extractor.rs:115`).
- `callables`: `Callable { name, parameters, file_path, .. }`.
- `assignments`: `AssignmentKey { scope, variable_name } -> Assignment { value /* raw RHS text */ }`.
- `ImportGraph.resolved_imports: (importer_file, codeword) -> ResolvedImport { source_file, .. }`.

Pass 2 order today: `resolve_endpoint_handlers` (Go) -> entity fields -> call argument types -> edge identification -> `post_process_endpoints` (Python urlpattern enrichment).

Corpus shapes the first adapter must cover:
- empaia: `app.mount("/v1", app_v1)` + `add_routes_v1(app_v1)` (app.py) -> `def add_routes_v1(app): add_routes_clinical(app)` (api/v1/__init__.py) -> `def add_routes_clinical(app): @app.get(...)` (api/v1/clinical.py). 36 mounts in 8 services; one keyword form `app.mount(path="/v3/fhir", app=app_v3)`.
- polyglot: `BASE_PATH = "/api/v1/consignservice"` (module level, routes.py) + `router = APIRouter(prefix=BASE_PATH)` inside `create_router` + `@router.post(...)`.

## Goals / Non-Goals

**Goals:**
- One resolver, no language or framework names in it.
- Adapters are small and pure: `&TypedFileRecord -> Vec<RouteFact>`.
- Deterministic output, independent of file order.
- Never worse than today: anything unresolved keeps the current URI.

**Non-Goals:**
- Adapters other than Python FastAPI / Starlette. Flask (`Blueprint(url_prefix=)`, `register_blueprint`), Django `include()`, Go (`StripPrefix`, chi `Route`, `basePath + "/x"` route strings), Express: later changes, adapter only.
- Moving Spring class-level prefix handling out of the Java extractor.
- FastAPI `root_path` (deploy-time proxy prefix, not part of the route).
- Prefixes built from settings, env vars or f-strings.
- Router returned from a function and attached with an extra prefix (`include_router(create_router(s), prefix=...)`); not in corpora.
- Jev option text (endpoint file path): separate follow-up.

## Decisions

### D1. Neutral route facts in `models`
```rust
pub enum RouteFact {
    /// `child` is served under `parent` + `prefix`.
    Attach { scope: Scope, parent: String, child: String, prefix: String },
    /// `router` adds `prefix` to its own routes.
    Own { scope: Scope, router: String, prefix: String },
}
```
`prefix` is the raw expression text (literal with quotes, or identifier); the resolver evaluates it (D4). `scope` reuses `models::Scope` (`Global` / `Function(name)`).
- Alternative: store facts in `TypedFileRecord`. Rejected: they are derived from fields already there and used once in pass 2; adapters return them, the resolver consumes them.
- Alternative: one generic "call pattern" config table (YAML) instead of code adapters. Rejected for now: argument positions, keyword names and constructor-vs-call shapes differ per framework; a 30-line adapter is simpler than a pattern language. Revisit after 3 adapters.

### D2. `Endpoint.enclosing_function`
Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub enclosing_function: Option<String>` to `models::Endpoint`. Python decorator strategies fill it (reuse `find_enclosing_information`, `python-extractor/src/extraction/calls/extractor.rs:19`). Java, Go, Django set `None` (Go fills it with its adapter later).

### D3. Adapter dispatch and resolver placement
- `extractor-runtime/src/pipeline/pass2/route_prefixes.rs`: `pub fn resolve_route_prefixes(files: &mut [TypedFileRecord], import_graph: &ImportGraph)`.
- Adapters: `python_extractor::extraction::route_facts(&TypedFileRecord) -> Vec<RouteFact>`; dispatched by `file.language` like `endpoints.rs` strategies; Java/Go/Unknown -> empty.
- Called in `build_project_ir` right after `resolve_call_argument_types` (calls are typed, endpoints not yet post-processed), before `identify_edges`.

### D4. Resolution
Index once: facts by `(file, child/router)`, plain calls by callee name, callables by `(file, name)`.

`prefixes(file, scope, var, path) -> BTreeSet<String>` (`path` = visited `(file, scope, var)`, max depth 8):
1. `var` is parameter `i` (name `n`) of callable `scope` in `file`: union over every call to that callable (D5) of `prefixes(caller_file, caller_scope, arg, ..)` where `arg` is the `n=` keyword value or the `i`-th positional value. No caller -> `{""}`.
2. Else: `outer` = union over `Attach` facts with `child == var` (lookup scope first, then `Global`) of `{p + eval(prefix) | p in prefixes(file, fact.scope, parent, ..)}`; none -> `{""}`. Result = `{o + eval(own(var)) | o in outer}`.
3. On cycle or depth > 8: `{}` for that branch. Final empty set -> `{""}`.

`eval(expr)`: quoted literal -> content; identifier -> `Scope::Global` assignment in the same file, else the file that `ImportGraph` resolves the identifier to; else unresolved. Any unresolved prefix on the chosen path -> endpoint keeps its current URI.

Join: `a.trim_end_matches('/') + "/" + b.trim_start_matches('/')`, empty parts skipped, leading `/`.

Endpoint input: `router_variable` + `enclosing_function` (or `Global`) + `file_path`. Endpoints without `router_variable` are untouched (Java, Go, Django today).

### D5. Callee resolution
For a call whose `function_name` last segment is `f` in file `F`:
1. `ImportGraph.resolved_imports[(F, f)].source_file` -> callables named `f` there;
2. else callables named `f` in `F`;
3. else callables named `f` whose file shares the longest common path-component prefix with `F` (ties keep all).
Step 3 is the safety net if the import graph misses a relative import; it never mixes services because each service's directory is the longest shared prefix.

### D6. One endpoint per prefix
Emit one clone per prefix with `uri = join(prefix, uri)`; `{""}` leaves the endpoint untouched. Order: original endpoint order, prefixes in `BTreeSet` order.

## Risks / Trade-offs

- [Naive Python argument splitting breaks on commas inside nested calls] -> child does not match; endpoint keeps today's URI. No such mount in corpora.
- [Adapter quality varies per language] -> resolver tests use hand-written facts, so resolver correctness is independent of any adapter.
- [Endpoint count grows (one per prefix)] -> bounded by mounts; empaia v1/v3 duplicates already exist today as separate endpoints with identical URIs.
- [`Endpoint` JSON gains a field] -> skipped when `None`, `serde(default)` on read.
- [Running the resolver before `post_process_endpoints`] -> Python urlpattern enrichment clones per method after prefixes; Django endpoints have no router facts, so no interaction.

## Migration Plan

None. Extraction output only; rerun the CLI. Rollback = revert the commits.
