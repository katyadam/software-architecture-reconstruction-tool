## 1. Baseline

- [x] 1.1 With `route-prefix-eval-pass3` in the working tree, record train-ticket and polyglot REST targets and connections, plus 5 empaia runs, into `jev_triage_runs/python-inline-url-concat/baseline/`; verify the outputs exist

## 2. Implementation

- [x] 2.1 Add the concatenation branch to `identify_target_uri` per design D2-D5; verify unit tests in `python-extractor/tests/python/restcalls.rs` for constant + literal, single quotes, literal between names, f-string, literal with `+`, dictionary lookup (no call), literal first, bare variable and plus inside a nested call
- [x] 2.2 Add pipeline tests through pass 3 for the resolved scenarios: constant + literal, single quotes, literal between names, f-string, unresolved operand equals variable form, same constant name in two services in both file orders; verify they pass in `extractor-runtime/tests/pipeline/framework_endpoints.rs`
- [x] 2.3 Add inline-form tests next to the `url = BASE + "/x"` tests from `route-prefix-eval-pass3`; verify that a prefix and an inline REST call using the same `BASE` get the same value

## 3. Regression

- [x] 3.1 Re-run the corpora and diff against 1.1; verify train-ticket is identical; polyglot edges and F1 are identical, with only the 3 sites in design Risks changed; no empaia site reaches the new target form (AST scan), and no empaia REST target falls outside the union of pre-change runs; explain any difference
- [x] 3.2 Update the `Known limits` section of `docs/endpoint_restcall_matching.md` if it mentions this limit, and run `cargo test --workspace`; verify 0 failures
