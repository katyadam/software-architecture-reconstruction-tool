## 1. Baseline snapshot

- [x] 1.1 Before any edit, run `agentic-cli` on empaia without `--jev`. Keep the output in the scratchpad directory as a reference. Verify score.py gives 0.941.

## 2. Static matcher changes

- [x] 2.1 Remove the fuzzy fallback from `http/mod.rs`:
  - `fuzzy_fallback`, `fuzzy_distance`, `DISSIMILARITY_PERCENT`, `MatchTag::Fuzzy`
  - Update the HTTP tests that relied on fuzzy.
  - Test: `/v3/case` against `/v3/cases` gives no candidate.
  - Verify `cargo test -p agentic-matcher`.
- [x] 2.2 `group::link` takes the `MatchTag` and writes the `static` fact (`f1`, the tag in lower case, `<service> | <address> <- <target>`).
  - Test: a template link carries that fact.
- [x] 2.3 Split `match_static` into `plan(.., jev)` and `Plan::finish(links, unmatched)`. With `jev`, non-strong HTTP initiators go to `plan.pending` and get no link.
  - `match_static` stays `plan(.., false).finish(..)`.
  - Test: with `jev`, a single template candidate is linked, while a tie, a suffix-only call and a no-candidate call are pending.
  - Test: a Kafka producer is never pending.
- [x] 2.4 Run `agentic-cli` on empaia without `--jev`. Verify score.py gives TP 15, FP 1, FN 2 (F1 0.909), and that the only lost edge is `app-service -> medical-data-service`. After `http://mds` was added to the empaia configs: TP 15, FP 0, FN 2 (P 1.000, F1 0.938).

## 3. Extraction keeps the project

- [x] 3.1 `agentic/cli/src/extract.rs` returns `Extracted { project, ir, bindings }`, with the bindings keyed by point id. Use `evaluate_with_bindings`.
  - Test: on a small fixture, a REST call's point id maps to its bindings.
  - Verify `cargo test -p agentic-cli`.

## 4. Jev module in the matcher

- [x] 4.1 Add `agent-loop`, `agentic-tools` and `extractor-runtime` to the matcher's dependencies, and to the root workspace dependencies where they are missing. Make `agentic-tools`' `row` public.
  - Verify `cargo build -p agentic-matcher`.
- [x] 4.2 `jev/wording.rs`: the `Stage` enum (`Service`, `Endpoint`) with log stage names, the two `JevWording`s, and the `JevFactory` trait.
- [x] 4.3 `jev/task.rs`: the service and endpoint task text, as in the design.
  - Test: the text for a call with two candidates and one binding.
  - Test: the text for a call with no candidate.
- [x] 4.4 `jev/answers.rs`: the service answer set, and the endpoint answer set with the method filter and the 254 cap.
  - Test: a service with no receiver of the protocol is not offered.
  - Test: the method filter.
  - Test: 300 receivers are cut to the 254 closest.
- [x] 4.5 `jev/outcome.rs`: map the two sessions to a link or an `Unmatched`, per the spec table. Renumber the evidence and fill `tried`.
  - One test per table row.
  - Test: renumbering across both sessions.
- [x] 4.6 `jev/mod.rs`: `resolve(pending, tool ctx, bindings, factory, log) -> JevResult`. For each pending point in point id order, run the service session, then the endpoint session when it applies, and collect the links, the unmatched and the `JevMetrics`.
  - Test with a scripted model: an endpoint pick, a service-only answer, `NoFit` and a model error, end to end through `finish`.
  - Test: a second run on the same log replays every session and makes no model call.
- [x] 4.7 `Metrics` gains `jev: Option<JevMetrics>`, skipped when `None`. The per-protocol `links` includes Jev links.
  - Test: the counts from 4.6 add up.

## 5. CLI

- [x] 5.1 Add `--jev` to `agentic-cli`:
  - `#[tokio::main]`
  - build `TypeSafeClient::new()` before extraction, failing early without a key
  - a factory that builds `JevModel`
  - open `match/sessions.jsonl`
  - `plan` -> `resolve` -> `finish` -> write
  - Verify that without `--jev` the empaia outputs equal those from 2.4, byte for byte.
  - Verify that with `--jev` and no key, the run fails and writes nothing.
- [x] 5.2 Run `cargo clippy --all-targets --no-deps -- -D warnings` on the touched crates, and `cargo test` on the workspace. Verify no new warnings and no failing tests.

## 6. Measure (needs the user's Jev key)

- [x] 6.1 The user runs `agentic-cli --jev` on empaia. Record the score.py P/R/F1, the `jev` metrics and the tokens in the PR. Rerun once and verify every session replays.

  - Result (new config): TP 17, FP 1, FN 0, P 0.944, R 1.000, F1 0.971. 154 Jev initiators, 303 sessions, 1146 model calls, 5.5M input and 0.7M output tokens. A rerun replays every session with byte-identical output.
  - Remaining FP `workbench-service -> wsi-service`: an extraction gap (adjacent f-strings), not an oracle error.
