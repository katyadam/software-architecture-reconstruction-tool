## 1. Tests first

- [x] 1.1 Add e2e tests in `extractor-runtime/tests/pipeline/framework_endpoints.rs` for each scenario of `Go closure routers produce prefixes`, and verify they fail for the expected reason
- [x] 1.2 Rewrite `closure_parameter_not_extracted` in `go-extractor/src/extraction/tests.rs` to the `s.Register(func(r chi.Router) {...})` form, and update `chi_group_closure_ignored` in `route_facts/mod.rs` to expect the empty-prefix fact

## 2. Implementation

- [x] 2.1 Add `closure_routers` and `scoped_router` (design D2) with unit tests for nesting, shadowing and a receiver chain; verify with `cargo test -p go-extractor`
- [x] 2.2 Emit closure `Attach` facts and scope the `Mount` parent (D3); verify adapter tests pass
- [x] 2.3 Pass closures through `ExtractParams` and use them in `ChiStrategy` (D4); verify all 1.1 tests pass

## 3. Verification

- [x] 3.1 Run `cargo test --workspace` and `cargo clippy --workspace`; verify no failures or new warnings
- [x] 3.2 Run the CLI on train-ticket-polyglot and train-ticket; verify endpoints and connections match `jev_triage_runs/python-inline-url-concat/after` (polyglot 264 endpoints, 89 edges)
- [x] 3.3 Run the CLI on empaia, which has no Go files, and compare endpoint URIs only, since its scores vary per run (#62); verify the URI set is unchanged
- [x] 3.4 Reviewer, Security and QA reports in `agents/reports/chi-route-closures-<role>.md`; resolve findings
- [x] 3.5 Update `openspec/backlog-route-prefixes.md` item 3 and record the non-goals as a GitHub issue if the user wants them tracked (#69, #70)
