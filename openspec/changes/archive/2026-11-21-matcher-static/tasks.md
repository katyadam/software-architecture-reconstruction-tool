## 1. Shared classification

- [x] 1.1 Move `synthesizer/src/sdg/interaction_kind.rs` into `models/src/interaction_kind.rs` (`classify`, `InteractionSignals`, `is_reflexive`, `host_of`, helpers). Replace the synthesizer module with `pub use models::interaction_kind::*;`. Verify `cargo test -p models -p synthesizer` passes with no test edits.

## 2. Matcher crate

- [x] 2.1 Create `agentic/matcher` (`agentic-matcher`), depending on `agentic-models` and `models`. Verify `cargo build -p agentic-matcher`.
- [x] 2.2 `points.rs`: `Option`-returning service assignment (`models::configuration::assign_service`, shared with synthesizer) and `Placed` points for calls and endpoints, with `Address::Http` from the substituted target. Unit tests: assigned, unassigned, and the address split. Verify `cargo test -p agentic-matcher`.
- [x] 2.3 `models::substitute::Substituter`: moved from `substitute_constants_in_restcalls`, which now calls it. Tests: a hole, a bare word, and `HOST` not matching inside `HOSTNAME`. Verify the tests pass.
- [x] 2.4 `candidates/`: copy `candidates()` and its helpers from `synthesizer/src/sdg/matching.rs`, over `Placed`, split into `mod.rs` and `segments.rs`. Copy its unit tests into `candidates/tests.rs`. Verify all copied tests pass.
- [x] 2.5 `group.rs`: one `Link` per candidate (`Static`, no confidence, no evidence), `request_kind` copy, dependencies grouped by `(source, target)` in a `BTreeMap`, kind rollup, `NoCandidate` unmatched. Tests: two candidates give two links, rollup, no candidate. Verify the tests pass.
- [x] 2.6 `metrics.rs`: counts per the spec. Test: one same-service tie and one cross-service tie are counted apart. Verify the test passes.
- [x] 2.7 `lib.rs`: `match_static(endpoints, calls, config, constants) -> (Matches, Metrics)` with sorted services. Verify `cargo test -p agentic-matcher` and `cargo clippy -p agentic-matcher --no-deps -- -D warnings`.

## 3. CLI crate

- [x] 3.1 Create `agentic/cli` (bin `agentic-cli`) with flags `-p`, `-c`, `-f` (optional), `-o`. `extract.rs` runs passes 1–3 with no LLM; `collect_files` and `collect_syntactic_records` are shared from `extractor_runtime::pipeline::files`. Verify `cargo build -p agentic-cli`.
- [x] 3.2 `legacy.rs`: `Matches` -> synthesizer JSON shape, with the original element as `restcall`. Test: one link round-trips to `connections[0].requests[0].restcall.file_path`. Verify the test passes.
- [x] 3.3 `output.rs`: write the `match/` and `final/` files as pretty JSON. Verify `cargo clippy -p agentic-cli --no-deps -- -D warnings`.

## 4. Acceptance

- [x] 4.1 Run `agentic-cli -p ../empaia -c config/configurations/local-empaia-config.json -f config/constants/empaia-constants.json -o results/agentic-empaia-static`. Score `final/sdg.legacy.json` with the zenodo score.py against `empaia-ground-truth/sdg-edges.json`. Verify P/R/F1 = 0.941/0.941/0.941, with the FP and FN the spec names.
- [x] 4.2 Run 4.1 a second time into another directory and `diff -r` the two. Verify no difference.
- [x] 4.3 Record the fan-out counts from `metrics.json` in this task. Run `openspec validate matcher-static --strict`. Verify no errors.
  - empaia, 2026-11-21: 577 initiators. 126 no candidate, 434 single, 15 same-service tie, 2 cross-service tie. 1210 receivers, 0 unassigned. 476 links, 17 dependencies.
  - Score: TP 16, FP 1, FN 1, P/R/F1 0.941. Two runs byte-identical.
