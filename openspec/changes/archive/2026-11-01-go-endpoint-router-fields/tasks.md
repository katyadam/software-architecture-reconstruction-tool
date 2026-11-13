Implement with a Code Writer subagent (`agents/writer/CLAUDE.md`); the orchestrator does not edit source. Read `design.md` (D1-D3) and the delta spec first. Work on branch `feat/endpoint-route-prefixes` (PR #55).

Global constraints (every task):
- No `unwrap()` outside tests; no `#[allow(...)]`; `->` never the Unicode arrow; comment density like the surrounding code.
- Change only `go-extractor`.
- `cargo fmt -p go-extractor`; `cargo clippy -p go-extractor` adds no new warnings.
- Commit trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. `openspec/` stays uncommitted.

## 1. Fields

- [x] 1.1 Failing tests in `go-extractor/src/extraction/tests.rs`, one per spec scenario, asserting `(router_variable, enclosing_function)`: `go_chi_endpoint_records_router_and_function` (`Some("r"), Some("routes")`), `go_method_endpoint_records_bare_function_name` (`Some("mux"), Some("register")`), `go_gorilla_endpoint_records_inner_receiver` (`Some("r"), Some("setup")`), `go_package_level_registration_has_no_router` (`None, Some("main")`), `go_closure_registration_uses_outer_function` (`Some("r"), Some("routes")`). Each also asserts URI and method. Verify: `cargo test -p go-extractor` shows these 5 failing, others green.
- [x] 1.2 Implement D1 (`EndpointMatch.router`, shared `router_variable` helper, set in chi, gorilla, serve_mux x2, web) and D2 (parent walk in `endpoints.rs`, both fields set in `build_endpoint`). Verify: `cargo test -p go-extractor` all green, including existing endpoint tests unchanged.
- [x] 1.3 Full suite: `cargo test -p models -p python-extractor -p java-extractor -p go-extractor -p extractor-runtime -p clients -p synthesizer -p cli` green.

## 2. Regression and commit

- [x] 2.1 Static CLI run on `../train-ticket-polyglot` (config `./config/configurations/local-train-ticket-polyglot-config.json`) into `jev_triage_runs/go-router-fields/polyglot/`. Compare with `jev_triage_runs/prefix-v1/prefix-train-ticket-polyglot/sdg.json`: the sets of `(service, http_method, uri)` across `services[].endpoints` are equal (264), and `connections` are equal. Also confirm at least one Go endpoint now has a non-null `router_variable`. Verify: comparison script prints equal sets and the Go count.
- [x] 2.2 Commit `feat(go-extractor): endpoints carry router variable and enclosing function`.
- [x] 2.3 Backlog update in `openspec/backlog-route-prefixes.md`: tick item 1. Under item 2, add: chi/ServeMux drop parameter routers today (`shared.rs` receiver checks), so the adapter must accept them; the resolver's `bare()` turns Go signatures (`func (s *S) f(...)`) into `func`, so normalize them. Verify: file shows both notes.
