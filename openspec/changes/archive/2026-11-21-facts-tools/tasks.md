## 1. Baseline snapshot

- [x] 1.1 Before any edit, run baseline `cli` on empaia with no LLM and `agentic-cli` on empaia. Put both runs in the scratchpad directory as references.

## 2. `evaluate` borrows

- [x] 2.1 `pipeline/pass3/mod.rs`:
  - `evaluate(&ProjectIR, ...)`
  - `evaluate_with_bindings` returns `(EvaluatedIR, Vec<OperandBindings>)`, with `bindings[i]` belonging to `restcalls[i]`
  - `OperandBindings` is `pub`
  - Update `extractor-runtime/src/api/service.rs`, the extractor-runtime tests and `agentic/cli/src/extract.rs`.
  - Test: the bindings line up with the restcalls on a fixture.
  - Verify `cargo test -p extractor-runtime`.

## 3. Neutral facts

- [x] 3.1 Add `Subject` with `rest_call(rc, bindings, ir)` and `message(edge, ir)`. Move the receiver, enclosing-class and snippet logic from `residual_call_state` into `Subject::rest_call`; `residual_call_state` reads them from the subject.
  - Test: on the fixture, the subject's fields equal the old `ResidualCallState` fields.
  - Test: a message destination `Queues.queueName` gives the symbol `queueName`.
- [x] 3.2 Move `llm_enhance/facts/` to `pipeline/facts/` as `pub mod facts`. `FactCtx` holds `subject` in place of `rc`, `bindings` and `state`. `fetch` returns `(id, body)`, and `dispatch.rs` wraps it into its `Fact`. Split the test fixtures: the project and config go to `pipeline/facts/test_fixtures.rs`, the DDFA state stays in `llm_enhance`.
  - Verify the existing facts and DDFA tests pass with unchanged expected ids and bodies.
  - Verify `cargo clippy -p extractor-runtime --all-targets --no-deps -- -D warnings` shows no new warnings.

## 4. Crate `agentic/tools`

- [x] 4.1 Scaffold `agentic-tools` with `ToolCtx`, `PointTools::new` and `impl agent_loop::Tools`.
  - Code facts go through `pipeline/facts`, with `Explored` in a `RefCell`.
  - Options come in spec order.
  - Test: on the fixture call, the code fact ids match DDFA's.
  - Test: a taken option is not offered again.
  - Test: opening `settings.py` offers `config.py`.
- [x] 4.2 `receivers.rs`: `similar` and `service:<name>`, with rows `id | service | address`.
  - Test: other service only, method filter, protocol kept, own service offered, 55 rows give 40 plus `... 15 more`, and an empty result gives `(nothing found)`.
- [x] 4.3 `imports.rs`: imported files with codewords, and importing files, project-relative.
  - Test: both directions on the fixture.
  - Verify `cargo test -p agentic-tools` and `cargo clippy -p agentic-tools --all-targets --no-deps -- -D warnings`.

## 5. CLI and acceptance

- [x] 5.1 ~~`agentic/cli/src/extract.rs` returns the `ProjectIR`, the `EvaluatedIR` and the bindings.~~ Deferred to `matcher-jev` (2026-11-21). Nothing would read them yet, so a binary crate warns "field never read". `extract.rs` only passes `&ir`.
- [x] 5.2 Rerun baseline `cli` on empaia with no LLM. Verify `sdg.json` holds the same elements as the 1.1 reference, ignoring order. Two runs of the same baseline already differ in order (checked 2026-11-21).
- [x] 5.3 Rerun `agentic-cli` on empaia. Verify `diff -r` of `match/` and `final/` against the 1.1 reference shows no difference, and that score.py gives 0.941.
- [x] 5.4 Run `cargo test` for the workspace. Run `openspec validate facts-tools --strict` and verify no errors.
