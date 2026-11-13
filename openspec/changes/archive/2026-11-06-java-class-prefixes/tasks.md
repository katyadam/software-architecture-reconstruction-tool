## 1. Baseline and tests first

- [x] 1.1 Before any code change, run the CLI on train-ticket, train-ticket-polyglot and empaia, and save the sorted endpoint URI sets with counts under `jev_triage_runs/java-class-prefixes/before`; verify each file is non-empty
- [x] 1.2 Add e2e tests to `extractor-runtime/tests/pipeline/framework_endpoints.rs` for every scenario of the six ADDED requirements, except the corpus one. Verify they fail for the expected reason, such as `MediaType.APPLICATION_JSON/{id}` or `GET` instead of `POST`
- [x] 1.3 Add a pass-2 unit test in `pass2/route_prefixes.rs`: a router-less endpoint with a preset chain keeps it when other files have facts. Verify it fails first

## 2. Implementation

- [x] 2.1 Make pass 2 keep chains on endpoints without a router (design D2); verify 1.3 passes with `cargo test -p extractor-runtime route_prefixes`
- [x] 2.2 Add `extract_uris` and `method=` parsing in `java-extractor/src/extraction/endpoints/annotations.rs` (D5). Unit tests cover:
  - arrays
  - non-literal values
  - `method` with a single value or an array
  - `@PathVariable` not matching `@Path`

  Verify with `cargo test -p java-extractor annotations`
- [x] 2.3 In `endpoints/extractor.rs` and `queries.rs`, implement the following and verify with `cargo test -p java-extractor`:
  - prefixes from the enclosing type (D4)
  - prefix chains with qualified constants reduced to their last segment (D1, D3)
  - the Feign skip (D6)
  - one endpoint per method path × HTTP method (D5)
  - removal of `shared_uri`
  - updated `tests/java/endpoints.rs` expectations: the URI is the method path, and `route_prefixes` holds the class prefix
- [x] 2.4 Verify all 1.2 tests pass with `cargo test -p extractor-runtime --test pipeline framework_endpoints`
- [x] 2.5 Update `docs/endpoint_restcall_matching.md` known limits. Java class prefixes are now covered; B7, method-level constants and context-path (#66) remain. Verify the doc lists those three

## 3. Verification

- [x] 3.1 Run `cargo test --workspace` and `cargo clippy --workspace`; verify no failures and no new warnings
- [x] 3.2 Rerun the CLI on the three corpora and diff against 1.1. Verify the URI sets and counts are equal; empaia compares URIs only, because its scores vary per run (#62). Explain any diff
- [x] 3.3 Get Reviewer, Security and QA reports in `agents/reports/java-class-prefixes-<role>.md`; resolve the findings
- [x] 3.4 Comment on #71: B1-B6 done, B7 still open
