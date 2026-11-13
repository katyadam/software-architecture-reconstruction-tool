## 1. Tests first

- [x] 1.1 Add e2e tests in `extractor-runtime/tests/pipeline/framework_endpoints.rs` for each scenario of `Django include produces prefixes`, and verify they fail for the expected reason
- [x] 1.2 Add resolver unit tests in `pass2/route_prefixes.rs` for each scenario of `Router composition can cross files through a module reference`, and verify they fail for the expected reason

## 2. Implementation

- [x] 2.1 Add `RouteFact::AttachModule` (design D1) and make the resolver follow it (D2, D3); verify 1.2 passes and existing resolver tests stay green with `cargo test -p extractor-runtime route_prefixes`
- [x] 2.2 Add `python-extractor/src/extraction/route_facts/django.rs` (D4) with adapter unit tests for fragments, tuple, `namespace=`, `r`-literal, metacharacter regex, and non-literal module; register it in `FRAMEWORKS`; verify with `cargo test -p python-extractor` and that all 1.1 tests pass
- [x] 2.3 Update known limits in `docs/endpoint_restcall_matching.md` and item 4 of `openspec/backlog-route-prefixes.md`; verify `include()` is no longer listed as missing and the non-goals are listed

## 3. Verification

- [x] 3.1 Run `cargo test --workspace` and `cargo clippy --workspace`; verify no failures or new warnings
- [x] 3.2 Run the CLI on train-ticket-polyglot and train-ticket; verify endpoints and connections match `jev_triage_runs/python-inline-url-concat/after` (polyglot 264 endpoints, 89 edges)
- [x] 3.3 Run the CLI on empaia and compare endpoint URIs only, since its scores vary per run (#62); verify the URI set is unchanged
- [x] 3.4 Reviewer, Security and QA reports in `agents/reports/django-include-prefixes-<role>.md`; resolve findings
