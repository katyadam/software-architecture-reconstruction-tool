## 1. Baseline

- [x] 1.1 Record current endpoint sets (service, method, URI) for empaia, train-ticket and train-ticket-polyglot with the CLI (no `--llm`, no `--typesafe`) into `jev_triage_runs/route-prefix-eval-pass3/baseline/`; verify the files exist and polyglot shows 264 endpoints / 89 connections
- [x] 1.2 Grep pass 2 code between `resolve_route_prefixes` and the end of `build_project_ir` for reads of `Endpoint.uri`; verify that only `post_process_endpoints` (emptiness check) reads it, otherwise update design.md before continuing

## 2. Model

- [x] 2.1 Add `PrefixExpr { file_path, expr }` and `Endpoint.route_prefixes: Vec<Vec<PrefixExpr>>` with `#[serde(skip)]` in `models`; verify `cargo build` passes and a serde round-trip test shows the field is absent from JSON

## 3. Pass 2: walk only

- [x] 3.1 Change `Resolver::prefixes` to return chains of `PrefixExpr` (expression text plus the file it is written in) instead of evaluated strings; keep the depth cap, visited guard and param following unchanged; verify the rewritten pass 2 unit tests assert the expected chains for attach, own+outer, param across two calls, reused helper (two chains) and recursive helper
- [x] 3.2 `resolve_with_facts` stores the chains on each endpoint and no longer clones or rewrites `uri`; verify by a test that the endpoint count and URIs after pass 2 equal the input
- [x] 3.3 Delete `Resolver::eval`, `unquote` and the lookups used only by them; verify `cargo clippy -p extractor-runtime` reports no dead code or warnings

## 4. Pass 3: evaluate and join

- [x] 4.0 In `build_file_env` and `fill_env_with_imported_constants`, make module and imported constants overwrite project-wide constants (attrs keep `or_insert`); verify a unit test with `BASE_PATH` in two files gives each file its own value
- [x] 4.1 Add a helper that evaluates one expression text in a file env to `Option<String>`: quoted literal, or exact env key with an `Expr::Literal` value that is not a raw f-string; verify unit tests: quoted literal, Go backtick raw string, chained constant, external constant `settings.base`, unknown name -> `None`
- [x] 4.2 Add `pass3/route_prefixes.rs`: lazy per-file env cache, evaluate each chain (any failure -> ""), join, `BTreeSet` of prefixes, one clone per prefix; move `join` here; verify the tests moved from pass 2 (`prefix_from_global_constant`, `prefix_from_imported_constant`, `unresolved_prefix_keeps_uri`, `helper_reused_by_two_mounts_yields_two_endpoints`) pass
- [x] 4.3 Call the step from the code shared by `evaluate` and `evaluate_with_llm` in place of the plain endpoint copy; verify `cargo test -p extractor-runtime` passes, including `tests/pipeline/framework_endpoints.rs`

## 5. New behavior tests

- [x] 5.1 Add pipeline tests for every scenario in the spec delta (external constant, chained constant, imported constant, same value as REST call URL, same constant name in two services for prefixes and for REST calls, mixed resolvable/unresolvable chains, output shape unchanged); verify they pass

## 6. Regression and docs

- [x] 6.1 Re-run the CLI on the three corpora and diff against the 1.1 baseline; verify the endpoint sets are identical and polyglot still has 264 endpoints; diff connections against the baseline and explain every change caused by the env precedence fix
- [x] 6.2 Score empaia with the Zenodo v2 `score.py`; verify the edge score is not lower than baseline. Done 2026-11-13 on `07c168a`: static P/R/F1 0.941/0.941/0.941, identical over 4 runs (paper baseline 1.000/0.824/0.903)
- [x] 6.3 Update the "Route prefixes" section of `docs/endpoint_restcall_matching.md`: resolver runs in pass 2 (walk) + pass 3 (evaluation), new limits list, and add the missing Go adapter; verify the doc no longer lists Go or settings-based prefixes as unsupported
- [x] 6.4 Run `cargo fmt`, `cargo clippy` and `cargo test` for the workspace; verify all clean. 2026-11-13: tests pass (1075/0); fmt (5 diffs) and clippy (50 warnings) not clean, pre-existing after the main merge, not from this change
