Implement with a Code Writer subagent (`agents/writer/CLAUDE.md`); the orchestrator does not edit source. Read `design.md` (D1-D5) and the delta spec first. Work on branch `feat/endpoint-route-prefixes` (PR #55).

Global constraints (every task):
- No `unwrap()` outside tests; no `#[allow(...)]`; `->` never the Unicode arrow; comment density like the surrounding code.
- Change only `go-extractor` and `extractor-runtime`.
- `cargo fmt`; `cargo clippy -p go-extractor -p extractor-runtime` adds no new warnings (2 pre-existing in go-extractor).
- Commit trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. `openspec/` stays uncommitted.

## 1. Resolver

- [x] 1.1 D3: `bare()` in `extractor-runtime/src/pipeline/pass2/route_prefixes.rs` handles Go signatures. Unit test `bare_go_signatures`: `func (s *Server) routes(r chi.Router) http.Handler` -> `routes`, `func main()` -> `main`, `func Map[T any](x T)` -> `Map`, `create_router() -> Any` -> `create_router`. Verify: `cargo test -p extractor-runtime route_prefixes` green.
- [x] 1.2 Resolver unit test `go_group_param_across_files` using `rec`/`call`/`callable` helpers with Go-shaped names: `main.go` fact `Attach(r, api, "\"/api\"")` in scope `Function("func main()")`, call `registerRoutes(api)` enclosed by `func main()`; `routes.go` callable `registerRoutes` (param `g`), endpoint `/health` router `g`, function `registerRoutes`. Expect `/api/health`. Verify: test green.

## 2. Adapter

- [x] 2.1 D1 + D2: `go-extractor/src/extraction/route_facts/mod.rs` with `pub fn route_facts(file: &TypedFileRecord) -> Vec<RouteFact>`; export via `go-extractor/src/extraction/mod.rs`. Unit tests built from `extract_syntactic(..).into()`: `group_assignment` (Gin `api := router.Group("/api")` -> `Attach(router, api, "\"/api\"")`, scope is a `Function` whose bare name is `startRest`), `group_with_middleware` (`r.Group("/v1", auth)` -> prefix `"\"/v1\""`), `gorilla_subrouter`, `chi_mount_identifier`, `mount_call_result_ignored`, `chi_group_closure_ignored` (`r.Group(func(r chi.Router) {})` -> no fact), `group_prefix_from_const` (`const apiBase = "/api"` -> prefix `"\"/api\""`). Verify: `cargo test -p go-extractor route_facts` green.
- [x] 2.2 Add `Language::Go => go_extractor::extraction::route_facts::route_facts(file)` in `facts_for`. Verify: `cargo build -p extractor-runtime`.

## 3. Extractor

- [x] 3.1 D4: replace `router_prefix` with a bool group-chain check in `endpoint_frameworks/shared.rs`; chi strategy uses the bare route path. Move `extracts_gin_group_routes` from `go-extractor/src/extraction/tests.rs` to `extractor-runtime/tests/pipeline/framework_endpoints.rs` as `gin_group_prefix_applied_once` (same code; expect `/api/customers/:id`, `/api/customer-basket`, `/api/health`, each once). In go-extractor add `gin_group_route_path_is_relative` (syntactic URI `/customers/:id`, router `routerGroup`). Verify: both green.
- [x] 3.2 D5: `is_router_parameter` in `shared.rs`; chi and ServeMux strategies accept router parameters. go-extractor tests, one per spec scenario: `chi_router_parameter_extracted` (`GET /status`, router `r`, function `routes`), `serve_mux_parameter_on_method` (`GET /items`, function `register`), `unrelated_parameter_type_ignored` (`c *Cache`, no endpoint), `gin_context_parameter_ignored` (`c *gin.Context`, `c.Get("/key", v)` -> no endpoint), `closure_parameter_not_extracted` (chi `Route` closure -> no `/x`). Verify: `cargo test -p go-extractor` green, existing tests unchanged.

## 4. Pipeline e2e

- [x] 4.1 In `framework_endpoints.rs`, one test per remaining spec scenario through the pass-2 pipeline (Go helper analogous to the Python `endpoint_uris`): `gin_nested_groups` (`/api/v1/items`), `gorilla_subrouter_prefix` (`/orders/{id}`), `chi_mount_named_subrouter` (`/admin/stats`), `echo_group_prefix_from_const` (`/api/carts`), `chi_mount_call_result_ignored` (route inside `adminRouter` keeps its own path), `gin_group_passed_to_helper_in_other_file` (`main.go` + `routes.go`, `/api/health`). Verify: `cargo test -p extractor-runtime --test mod` green.
- [x] 4.2 Full suite: `cargo test -p models -p python-extractor -p java-extractor -p go-extractor -p extractor-runtime -p clients -p synthesizer -p cli` green.

## 5. Regression and commit

- [x] 5.1 Static CLI run on `../train-ticket-polyglot` (config `./config/configurations/local-train-ticket-polyglot-config.json`) into `jev_triage_runs/go-route-facts/polyglot/`. Compare with `jev_triage_runs/prefix-v1/prefix-train-ticket-polyglot/sdg.json`: `(service, http_method, uri)` sets over `services[].endpoints` equal (264); `connections` equal ignoring `router_variable` / `enclosing_function`. Verify: comparison script prints equal.
- [x] 5.2 Same comparison for empaia (1210 endpoints) and train-ticket against their `jev_triage_runs/prefix-v1/` runs (Python/Java; must be unchanged by the `bare()` change). Verify: equal sets.
- [x] 5.3 Commit `feat: Go route-fact adapter, router parameters, resolver Go signatures`.
- [x] 5.4 Backlog `openspec/backlog-route-prefixes.md`: tick item 2 with commit; move the bare-path `HandleFunc`, call-result `Mount` and struct-field router notes to a "Go gaps" list. Verify: file shows them.
