## Why

Go endpoints only get a router-group prefix when the group and its routes sit in the same file (`shared.rs` `router_prefix`, scope-blind). A group built in `main` and passed to `registerRoutes(g *gin.RouterGroup)` in another file loses its prefix, and the routes inside such helpers are dropped outright because chi/gin/ServeMux reject parameter routers. The shared resolver already follows routers through calls for Python; Go only needs an adapter. Endpoints now carry router and function (`go-endpoint-router-fields`), so the adapter is unblocked.

## What Changes

- New Go route-fact adapter (`go-extractor/src/extraction/route_facts/`), idiom-table style like Python:
  - `x := r.Group("/p")` (Gin, Echo, Fiber) -> `Attach(r, x, "/p")`.
  - `x := r.PathPrefix("/p").Subrouter()` (gorilla) -> `Attach(r, x, "/p")`.
  - `r.Mount("/p", sub)` (chi, Fiber) -> `Attach(r, sub, "/p")`.
- Resolver wires `Language::Go` in `facts_for`.
- Resolver `bare()` normalizes Go signatures (`func (s *S) name(...)` -> `name`), so Go fact scopes and call-site scopes match endpoint `enclosing_function`.
- Endpoint strategies accept a router that is a parameter of the enclosing named function when its type comes from a known router package (`chi.Router`, `*gin.RouterGroup`, `*gin.Engine`, `*echo.Group`, `fiber.Router`, `*mux.Router`, `*http.ServeMux`).
- **BREAKING (syntactic level only):** the extractor no longer joins `.Group(...)` prefixes itself. `extract_syntactic` returns the route path relative to its router; the pipeline (pass 2) adds the prefix. Pipeline output for same-file groups is unchanged.

Non-goals (stay in backlog):
- chi `r.Route("/p", func(r chi.Router) {...})` closures (item 3). Routes on a closure parameter stay unextracted.
- `r.Mount("/p", subRouter())`: child is a call result, not a variable. Ignored.
- Routers held in struct fields (`s.router.Get(...)`).
- Bare-path `http.HandleFunc("/x", h)` without a method: endpoint extraction, not prefixes.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `endpoint-route-prefixes`: adds Go composition idioms to the agnostic resolution, and requires routes registered on a router parameter of a named Go function to be extracted and prefixed.

## Impact

- `go-extractor/src/extraction/route_facts/{mod,idiom}.rs` (new), `go-extractor/src/extraction/mod.rs` (export).
- `go-extractor/src/extraction/endpoint_frameworks/{shared,chi,serve_mux}.rs`: drop group-prefix joining, add parameter-router acceptance.
- `go-extractor/src/extraction/tests.rs`: `extracts_gin_group_routes` moves to the pipeline e2e test.
- `extractor-runtime/src/pipeline/pass2/route_prefixes.rs`: `Language::Go` arm, `bare()` Go signatures.
- `extractor-runtime/tests/pipeline/framework_endpoints.rs`: Go e2e cases.
- Regression: polyglot (264 endpoints, 89 connections), empaia (1210), train-ticket unchanged vs `jev_triage_runs/prefix-v1/`. No local corpus uses Go groups, so gain is shown by tests only.
