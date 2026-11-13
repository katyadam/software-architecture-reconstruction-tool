# Route prefixes: follow-up backlog (PR #55)

Branch `feat/endpoint-route-prefixes` -> `sage-rework`. Do items one by one, each as an OpenSpec change (`/opsx:propose` then `/opsx:apply`), committed onto the PR branch. Then merge PR #55 and exercise.

Base already in PR: `RouteFact` IR (`models/src/route_facts.rs`), resolver (`extractor-runtime/src/pipeline/pass2/route_prefixes.rs`), Python adapters as idiom tables (`python-extractor/src/extraction/route_facts/{idiom,fastapi,flask}.rs`).

## Items

1. [x] **Go endpoints carry router and function.** (done: `2f1acfa`, change `go-endpoint-router-fields`) Fill `Endpoint.router_variable` (receiver of `r.Get(...)`, `mux.HandleFunc(...)`) and `Endpoint.enclosing_function` (bare name) in `go-extractor/src/extraction/endpoint_frameworks/{chi,gorilla,serve_mux,web}.rs`. Today neither is set. Prerequisite for 2-3. No output change expected.
2. [x] **Go route-fact adapter.** (done: `d884ae8`, change `go-route-fact-adapter`; group joining dropped from extractor, router params accepted, `bare()` handles Go) New `go-extractor/src/extraction/route_facts` (idiom-table style, like Python): Gin/Echo/Fiber `x := r.Group("/p")` -> Own/Attach; chi `r.Mount("/p", sub)` -> Attach; gorilla `r.PathPrefix("/p").Subrouter()` -> Attach. Add `Language::Go` arm in `facts_for` (`route_prefixes.rs:26`). Decide: drop the same-file `.Group(` handling in `endpoint_frameworks/shared.rs:41-102` (`router_prefix`) in favour of the resolver, or keep it (risk: double prefix). Gains cross-function / cross-file routers.
3. [x] **chi `Route` closures.** (#63; done: `c6f0cb0`, change `chi-route-closures`) `r.Route("/p", func(r chi.Router) { r.Get(...) })`: inner router is a closure parameter, not a named function. Needs a fact or endpoint scope that ties closure endpoints to the outer router. Check how the Go extractor scopes closure endpoints first.
   - Note: today the inner `r.Get` IS extracted as `/x` without `/p`: `receiver_origin` is scope-blind and finds the outer `r := chi.NewRouter()`.
4. [x] **Django `include()`.** (#64; done: `07c35a0`, change `django-include-prefixes`) `path("api/", include("app.urls"))` and `re_path` now prefix the routes of module `app.urls`, found by walking up from the including file through new neutral fact `RouteFact::AttachModule`.
   - Not covered: `include(extra_patterns)` and inline `include([path(...)])`; DRF `include(router.urls)`; `include(module)` with an imported object; `ROOT_URLCONF`; normalizing a `re_path` child's own URI; legacy `url()`.
5. [x] **Update PR #55 description** (done 2026-11-04): idiom tables, Flask, plus each item above as it lands.

Go gaps (not prefix work, add when a corpus needs them; #65, alias bug #68):
- Closure gaps from `chi-route-closures` (#69), closure prefix missing: helper inside a closure (`users(r)`), named function (`r.Route("/p", users)`), `x := r.Group("/y")` inside a closure, `r.With(mw).Route`, second call of a chained `.Route`.
- Bare-path `http.HandleFunc("/x", h)` (no method prefix) yields no endpoint.
- `r.Mount("/p", subRouter())`: child is a call result; no fact.
- Routers in struct fields (`s.router.Get(..)`) are not extracted.
- Group var declared in another file of the package: fact stays function-scoped, prefix lost.
- Go import alias for `chi/v5` is `v5`; `imported_origin_matches` misses it (`is_router_parameter` works around it). Fix in Go import extraction.
- Shadowed router parameter in a nested block (`for _, r := range xs`) can yield a false endpoint.

Deferred (#66): Java Spring `server.servlet.context-path` (config-level, per-service prefix, not a `RouteFact`). Class-level `@RequestMapping`/`@Path` prefixes fixed in `ab1fc24` (#71, B1-B6; B7 open). Add only if a corpus needs it.

## Exercise notes

- No local corpus uses Go `Group`/`Mount`/`PathPrefix` or Django `include()` (polyglot Go services checked). Coverage = unit tests + `extractor-runtime/tests/pipeline/framework_endpoints.rs` e2e.
- Regression guard per item: empaia (1210 endpoints), polyglot (264), train-ticket unchanged vs `jev_triage_runs/prefix-v1/` runs. `prefix-v1/prefix-empaia` has no `sdg.json`; use `jev_triage_runs/go-route-facts/empaia/sdg.json` (run from repo dir with relative paths, else service names come out empty).
- Open vp PRs #29, #30 (gRPC) touch pass 2: expect merge conflicts, not overlap.
