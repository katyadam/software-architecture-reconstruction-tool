## 1. Model

- [x] 1.1 Add `ExcludedReason::InProcess` in `agentic/models/src/lib.rs`, with a doc comment. Verify a JSON round-trip test of an `InProcess` excluded point passes.

## 2. Answers and wording

- [x] 2.1 Replace `Pick = Option<String>` with `Option<Target>` (`Point`, `External`, `InProcess`) in `jev/wording.rs`. Update every use in the matcher and the CLI. Verify `cargo build -p agentic-cli` passes.
- [x] 2.2 Append the `external` and `in-process` options to `answers::services`, with descriptions per the spec. Verify the updated answer-set test lists services, then the two fixed answers.
- [x] 2.3 Add the two rules to the service question in `Stage::wording`. Verify with a test that the service wording names database layers, caches and SDK objects, and the own-service rule.
- [x] 2.4 Verify with a test that a configured service named `external` makes the session end in a model error.

- [x] 2.5 Read the call expression into the facts `Subject` (`call_span`), expose it as `PointTools::call`, and show it as a `code:` line in the service task. Verify the task tests show the flattened line, its absence, and the 300-char cut.

## 3. Outcomes

- [x] 3.1 Add `Resolved::Excluded` and map `External`/`InProcess` to it in `outcome::resolve`, with renumbered facts and a confidence note. Verify tests for "Cache lookup is in-process" and "Excluded keeps its facts".
- [x] 3.2 In `jev/mod.rs` `one()`, run the endpoint session only for `Target::Point`. Verify the "Vault call is external" test sees one session for the point.
- [x] 3.3 Add `JevResult.excluded` and the `JevMetrics.excluded` count by reason. Verify the "Counts add up" test with 6 initiators.

- [x] 3.4 Excluded notes list the static candidates' point ids after the confidence. Verify `non_service_picks_are_excluded_with_facts`.

## 4. Output

- [x] 4.1 `Plan::finish` takes the excluded points, already in point id order. The CLI passes them along and writes `match/excluded.json`. Verify a static test run writes an empty list.
- [x] 4.2 Run `cargo test -p agentic-models -p agentic-matcher -p agentic-cli -p agent-loop`, `cargo clippy` and `cargo fmt --check`. Verify all are clean.

## 5. Evaluation

- [x] 5.1 Static agentic run on empaia: verify `S/score.sh` matches the stored static run (P 1.000, R 0.941) and `excluded.json` is empty.
- [x] 5.2 Fresh `--jev` run on empaia (about 5 min, $0.24). Verify all 17 oracle edges are still found, and own-service picks in source files drop from 114 to near 0. First run, without the `code:` line: 17/17 kept, own 114 -> 73, `NoFit` 2 -> 13. With it: 17/17 kept, own 114 -> 42, `NoFit` 6. Accepted at 42; the rest are DB access `*Client` classes, follow-up #91.
- [x] 5.3 Read `excluded.json` by hand: list any excluded point whose code calls a configured service's URL. Record the counts by reason and these findings in a comment on #87. Done: 65 `InProcess`, 7 `External`, all correct.
