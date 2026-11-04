# Endpoint-RestCall matching

## Problem

- How to solve a problem finding a REST call correct target endpoint?
- You can match directly (endpointUrl == restCallTargetUrl).
- This is not feasible, often endpoint doesn't contain full url specification.
- Also service can be deployed in a way that it is accessible by multiple URLs - docker network, localhost, etc...

## Solution

- Matching unifies path segments (`synthesizer/src/sdg/matching.rs`); Levenshtein only as a last-resort fallback.
- Each service in configuration can contain multiple urls that it is accesible from.

### Algorithm

1. Method filter: only endpoints with the call's HTTP method.
2. Host scope: reflexive call (localhost / own url) -> own service only; host owned by a configured service -> that service; unknown host -> every other service.
3. Segment unification: same length, each segment pair equal or one side a variable (`{x}`, `:x`, `*`) -> `exact` (all literal) or `template`.
4. Suffix: call starting with a hole (`{HOST}/a/b`) or a known host with extra leading segments (lost router prefix, e.g. `/v3`) matches the endpoint on a trailing run -> `suffix`.
5. Evidence: drop a candidate with no aligned position literal on both sides (`{requestUrl}`, `{a}/{b}` unify with anything).
6. Suffix candidates dropped when any `exact` / `template` candidate exists.
7. Specificity pruning: drop a candidate whose literal-matched positions are a strict subset of another's.
8. Fuzzy fallback: nothing left -> Levenshtein argmin over the scoped endpoints (uri prefixed with each service url, or bare); kept if distance < 0.3 x the longer of endpoint uri and call uri; ties -> smallest sort key; at most one candidate, tag `fuzzy` (catches glued hosts like `http://svc:5000url`).
9. Deterministic sort by service name, uri, function name, file path.
10. One `Request` per surviving candidate, tagged in field `match` (`exact`, `template`, `suffix`, `fuzzy`).

## Route prefixes

Frameworks add URL prefixes outside the route decorator (`app.mount("/v1", app_v1)`, `include_router(r, prefix=...)`, `APIRouter(prefix=...)`). Extraction used to drop them, so empaia `/v1/...` and `/v3/...` endpoints collapsed to the same URI and collided in matching. Pass 2 now rebuilds the full URI before matching.

### Neutral facts

- `models::RouteFact::Attach { scope, parent, child, prefix }`: `child` is served under `parent` + `prefix`.
- `models::RouteFact::Own { scope, router, prefix }`: `router` adds `prefix` to its own routes.
- `prefix` is raw expression text (quoted literal or identifier); pass 3 evaluates it.
- `Endpoint.enclosing_function`: function containing the route registration (Python and Go fill it; others `None`).

### Resolver

Two steps, split at the pass boundary. Pass 2 finds the structure, pass 3 finds the values.

Walk (`extractor-runtime/src/pipeline/pass2/route_prefixes.rs`) runs in `build_project_ir` after `resolve_call_argument_types`, before `identify_edges` and `post_process_endpoints`. Input per endpoint: `file_path`, `enclosing_function` (or global scope), `router_variable`. Output: `Endpoint.route_prefixes`, a set of chains of unevaluated `PrefixExpr { file_path, expr }`, outermost first. The field is not serialized.

1. Variable is a parameter of its enclosing function -> follow every call site of that function; take the keyword argument with the parameter name, else the positional argument at the same index; recurse in the caller's file and scope. No callers -> no prefix.
2. Otherwise: `Attach` facts with `child == var` (enclosing scope first, then global) give parent chain + prefix, recursively; then each `Own` prefix of `var` is appended.
3. Callee resolution for a call to `f` from file `F`: import graph target of `f` in `F` -> `f` defined in `F` -> all files defining `f` with the longest common path prefix with `F` (ties keep all).
4. Depth cap 8 and a visited `(file, scope, var)` guard; a cut branch contributes nothing.
5. At most 256 chains per merge, first in sort order, with a warning. This bounds memory only: walk time stays exponential in mounts per level.

Evaluation (`extractor-runtime/src/pipeline/pass3/route_prefixes.rs`) runs in `evaluate` and `evaluate_with_llm`, using the file env that REST calls use (`pass3/env.rs`), for the file where each expression is written:

1. Quoted literal (`"`, `'`, backtick, no inner quote) -> content.
2. Otherwise exact env key, dotted keys included (`settings.base`). Own and imported constants beat same-named project constants; external-only names keep priority.
3. A name the file neither declares nor imports resolves only if all its project declarations share one value.
4. Values still holding source text and raw f-strings count as unresolved. Go values are stored unquoted, so path-like values from Go files are accepted.
5. One endpoint clone per distinct prefix (`BTreeSet` order), `uri = join(prefix, uri)`; join trims duplicate `/` and forces a leading `/`.
6. Unresolved expression in a chain -> that clone keeps the original URI.
7. Endpoints without chains (Java, Django) are untouched.

```
app.py:            app.mount("/v1", app_v1); add_routes_v1(app_v1)
api/v1/__init__:   def add_routes_v1(app): add_routes_clinical(app)
api/v1/clinical:   def add_routes_clinical(app): @app.get("/cases")
-> clinical.app -> param of add_routes_clinical -> __init__.app -> app.py app_v1
-> Attach(app, app_v1, "/v1") -> /v1/cases
```

### Adding an adapter

New language:

1. Write `fn(&TypedFileRecord) -> Vec<RouteFact>` in the language extractor crate, reading `call_statements` and `assignments`.
2. Add one match arm to `facts_for` in `route_prefixes.rs`, keyed on `file.language`.

New Python framework (`python-extractor/src/extraction/route_facts/`):

1. Add `<framework>.rs` with `ATTACH: [AttachCall; N]` and `OWN: [OwnCtor; M]` tables and `facts(file) = facts_from_idioms(file, &ATTACH, &OWN)`; or a custom `facts` fn if the idiom does not fit.
2. Add `<framework>::facts` to `FRAMEWORKS` in `route_facts/mod.rs`.

- `AttachCall { method, child: (kw, pos), prefix: (kw, pos) }`: `X.method(...)` -> `Attach(X, child, prefix)`; keyword first, then positional index (`None` = keyword only); missing prefix -> `""`.
- `OwnCtor { ctor, prefix_kw }`: `r = ctor(..., prefix_kw=...)` or `r = mod.ctor(...)` -> `Own(r, prefix)`, in the assignment's scope.

The resolver has no framework names; its tests use hand-written facts.

### Current adapters

Python (`python-extractor/src/extraction/route_facts/`), all frameworks run on every Python file:

FastAPI / Starlette (`fastapi.rs`):

- `X.mount(path, app)`, positional or keyword -> `Attach(X, app, path)`.
- `X.include_router(router, prefix=...)` -> `Attach(X, router, prefix)`; prefix keyword only, missing -> `""`.
- `r = APIRouter(prefix=...)` -> `Own(r, prefix)`.

Flask (`flask.rs`):

- `X.register_blueprint(bp, url_prefix=...)` -> `Attach(X, bp, url_prefix)`; prefix keyword only, missing -> `""`.
- `bp = Blueprint(name, __name__, url_prefix=...)` -> `Own(bp, url_prefix)`.

Django (`django.rs`, module-level calls only):

- `path(p, include("a.b"))`, `re_path(r, include("a.b"))` -> `AttachModule(urlpatterns, ["a/b.py", "a/b/__init__.py"], urlpatterns, p)`. Tuple form and `namespace=` included. The resolver takes the first match walking up from the including file's directory, so a same-named module in a sibling service never matches.
- `re_path` prefix: literal with leading `^` stripped; other regex metacharacters -> no fact.

Go (`go-extractor/src/extraction/route_facts/`):

- `x := r.Group("/p")` (Gin, Echo, Fiber) -> `Attach(r, x, "/p")`.
- `x := r.PathPrefix("/p").Subrouter()` (gorilla) -> `Attach(r, x, "/p")`.
- `r.Mount("/p", sub)` (chi, Fiber) -> `Attach(r, sub, "/p")`.
- Same-file constants are inlined as quoted literals.

### Known limits

- No Go `StripPrefix` or Express adapters yet.
- Django `include()` adds no prefix for: `include(extra_patterns)` or inline `include([path(...)])`, DRF `include(router.urls)` (its routes are not extracted), `include(module)` with an imported object, third-party modules, prefixes holding `,` or `=`, `re_path` regexes with metacharacters. Legacy `url()` is ignored. `ROOT_URLCONF` is not read: a module nothing includes is served as declared. A `re_path` child's own URI is not normalized (`/api/^x/$`). A root-level `accounts/urls.py` can match a third-party `accounts.urls` in any service. An `include()` nested in an inline or list-variable include gets only its own prefix. Converter syntax in a constant prefix (`API = "<int:org>/"`) stays as written. No fact for a comment between arguments, `u""` or implicit string concatenation, keyword `route=` / `view=` / `arg=`, or qualified `urls.include`.
- Spring class-level prefixes stay in the Java extractor.
- FastAPI `root_path` ignored (deploy-time proxy prefix).
- Prefixes from concatenation (`ROOT + "/x"`), env vars with no supplied value, or unresolved f-strings stay unresolved.
- REST calls keep first-wins for names not bound in the file, so prefixes and REST URLs can differ on such a collision.
- A constant re-exported through two files can depend on file order.
- Method calls (`self.f(app)`): `self` shifts positional argument indices; keyword arguments still match.
- Prefix walk stops after 100_000 steps per endpoint (`MAX_STEPS`, one warning); chains found so far are kept.
- Python arguments come from a naive comma split; commas inside nested calls or literals break the child or prefix match (endpoint keeps its URI).
