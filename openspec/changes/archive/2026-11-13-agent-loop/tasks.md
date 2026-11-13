## 1. Crate

- [x] 1.1 Create `agentic/agent-loop` (package `agent-loop`, edition 2024). Deps: `agentic-models`, `serde`, `serde_json`, `sha2`. Dev-dep: `tokio` (macros, rt). Add `agent-loop` to root `[workspace.dependencies]`. Check: `cargo check -p agent-loop`.
- [x] 1.2 Add the types from design.md: `ToolOption`, `Request`, `Step`, `Reply`, `Model`, `Tools`, `Outcome`, `Session`, `SessionKey`. Check: `cargo check -p agent-loop`.

## 2. Loop

- [x] 2.1 Implement `run_session` without the log: offer untaken options, number facts `f1..`, enforce the budget with a last tool-free call, reject unknown options and unknown cites as `ModelError`, map provider `Err` to `ModelError`, sum calls and tokens. Check: tests in 2.2 pass.
- [x] 2.2 Add a scripted fake `Model` and a fake `Tools`. Add one test per spec scenario: session keyed by stage and subject (point id and link id), tool then answer, answer at once, facts numbered, numbering restarts, unknown citation, same tool different files, taken option not offered, unknown option, budget reached, budget exhausted, budget zero, provider fails, tokens summed. Check: `cargo test -p agent-loop`.

## 3. Log and resume

- [x] 3.1 Implement the prompt hash (`sha256(model id \0 budget \0 task)`, hex). Check: a test shows a changed task or model id changes the hash.
- [x] 3.2 Implement `Log`: open in append mode, load the index, skip malformed lines with a stderr warning, append and flush one line per answered or budget-exhausted session, never log a `ModelError`. Check: tests "answer logged" and "model error not logged".
- [x] 3.3 Wire the log into `run_session`: replay on a key and hash hit with `replayed: true`, no model call. Check: tests "rerun after crash", "changed prompt reruns", "errored point retried", and a test with a truncated last line that still loads the earlier records.

## 4. Finish

- [x] 4.1 `cargo fmt`, plus `cargo clippy -p agent-loop` clean, with no `unwrap()` outside tests and no `#[allow(unused)]`. Check: commands exit 0 with no warnings for the crate.
- [x] 4.2 `openspec validate agent-loop --strict` passes. Check: command output.
