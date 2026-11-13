## Context

See proposal.md - Why. Current state:

- Go endpoints carry `router_variable` (receiver text) and `enclosing_function` (bare name) since `go-endpoint-router-fields`.
- `endpoint_frameworks/shared.rs::router_prefix` follows `x := base.Group("/p")` chains through same-file assignments (scope-blind) and joins the prefix into the path. chi strategy (which also serves Gin/Echo/Fiber selectors) requires `router_prefix(..)?` to succeed, so a receiver must trace to a constructor or a `Group` chain. ServeMux requires `is_serve_mux_receiver`. gorilla checks nothing.
- Go IR:
  - Local assignments: key scope `Scope::Function(signature)`, value = normalized source text for calls (`router.Group("/api")`, quotes kept).
  - Global `const`/`var` string values are stored evaluated, quotes stripped (`/api`).
  - Call statements: `function_name` = callee text (`r.Mount`), positional `arguments` (raw text), `enclosing_function_name` = signature.
  - Callables: `name` bare, `parameters` with `datatype` text (`*gin.RouterGroup`).
- Resolver (`route_prefixes.rs`): `bare()` cuts at the first `(`, so `func (s *S) f(..)` -> `func`. Pass 2 runs it before edge identification and endpoint post-processing.

## Goals / Non-Goals

**Goals:**
- One place adds Go group prefixes: the resolver.
- Routes on router parameters of named functions reach the resolver.

**Non-Goals:**
- Closure routers (item 3), call-result mounts, struct-field routers, cross-file Go constants (same package, no import edge).

## Decisions

### D1. Adapter = idiom tables over `TypedFileRecord`, like Python
`go-extractor/src/extraction/route_facts/mod.rs` exposes `route_facts(file: &TypedFileRecord) -> Vec<RouteFact>`. Two small tables:
- Assigned compositions (from `file.assignments`): value `<parent>.<method>(<prefix>[, ...])<suffix>` with `(method, suffix)` in `[("Group", ""), ("PathPrefix", ".Subrouter()")]` -> `Attach { scope: key.scope, parent, child: key.variable_name, prefix }`.
- Call compositions (from `file.call_statements`): `<parent>.Mount(prefix, child)` -> `Attach`, scope from `enclosing_function_name` (`Global` when none).
- Accept only when prefix is a quoted literal or identifier and child (Mount) is an identifier. Drops chi `r.Group(func(..){..})` (no prefix) and `r.Mount("/p", f())`.
- Prefix arg = text up to the first top-level `,` (Gin `Group("/p", mw)`).
- Alternative: one generic table shared with Python. Rejected: Python matches keyword args and constructor kwargs; Go is positional with chained suffixes. Two 30-line adapters beat one parametrized one.
- No per-framework files: one table covers Gin, Echo, Fiber, gorilla, chi; frameworks differ only in method name.

### D2. Adapter resolves same-file constants
Go globals are stored unquoted, and the resolver's `eval` only accepts quoted constants. So an identifier prefix is looked up first in the fact's scope, then `Scope::Global`, in `file.assignments`; a hit is emitted as `"<value>"`. A miss leaves the identifier (resolver keeps today's URI).
- Alternative: make the resolver accept unquoted constants. Rejected: Python stores non-string values unquoted too (`x = 3`), and they would become prefixes.

### D3. `bare()` understands Go signatures
`bare("func (s *Server) routes(r chi.Router) http.Handler")` -> `routes`: strip a leading `func `, then a leading `(...)` receiver, then cut at the first `(` or `[` (generics). Applies to fact scopes, call-site scopes and endpoint scopes alike. Python/Java names never start with `func `, so their keys are unchanged.
- Alternative: Go extractor stores `name` in `enclosing_function_name`. Rejected: `identify.rs` parses the signature from that field.

### D4. Extractor stops joining group prefixes
Replace `router_prefix(..) -> Option<String>` with a bool check: a receiver is a known router if its origin is a constructor/imported router package, or a `.Group(` / `.PathPrefix(..).Subrouter(` of a known router (recursive, cycle guard kept). Strategies use the bare route path. The resolver adds the prefix from the D1 fact.
- Alternative: keep joining and have the adapter skip same-scope groups. Rejected: two places own one rule, and the "same scope" test must match exactly or prefixes double.
- `extract_syntactic` now returns relative paths for grouped routes. Only pass 1 calls it; pass 2 resolves before anything reads URIs.

### D5. Router parameters accepted by declared type
New `shared.rs::is_router_parameter(function_node, params, types) -> bool`:
- Receiver must be a plain identifier.
- Nearest ancestor of kind `function_declaration | method_declaration | func_literal`; `func_literal` -> false (item 3).
- Its `parameters` hold a `parameter_declaration` naming the receiver. Type text, `*` stripped, is `<alias>.<Type>`; `alias` imports a package in the table and `Type` is in its allowlist:
  - chi: `Router`, `Mux`; gin: `Engine`, `RouterGroup`, `IRouter`, `IRoutes`; echo: `Echo`, `Group`; fiber: `App`, `Router`; httprouter: `Router`; gorilla mux: `Router`; net/http: `ServeMux`.
- Allowlist, not package match: `c *gin.Context` has `c.Get(key)` and must not become a route.
- chi strategy: known router receiver OR router parameter (non-ServeMux rows). ServeMux strategies: serve-mux receiver OR `http.ServeMux` parameter.

## Risks / Trade-offs

- [Same method name on many Go types (`routes`) merges their callers] -> union of prefixes, one endpoint per prefix; same behaviour as Python helpers. Acceptable.
- [Go same-package calls have no import edge] -> resolver falls back to longest shared path, which picks the package directory. Matches Go packaging.
- [Assignment reassigned in one function (`g := r.Group("/a"); g = r.Group("/b")`)] -> IR keeps the last value. Rare; accept.
- [Raw-string prefixes (`` `/p` ``)] -> resolver `unquote` rejects backticks; URI kept unprefixed. Adapter converts backticks to `"` if cheap, else ponytail note.
- [Corpus gains are invisible] -> no local corpus uses these idioms; coverage = adapter unit tests + pipeline e2e (`framework_endpoints.rs`).

## Migration Plan

None. Pipeline output for same-file groups is unchanged; rollback = revert commits.
