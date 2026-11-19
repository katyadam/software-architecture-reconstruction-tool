## 1. Remove citations

- [x] 1.1 `agentic-models`: remove `Evidence` and `Claim`; `Link`, `Unmatched`, `Excluded`, `Verdict` hold `evidence: Vec<Fact>`; update the round-trip tests. Verify `cargo test -p agentic-models` passes.
- [x] 1.2 `agent-loop`: add `Answer<A> { value, confidence, note }`; `Step::Final(Answer<A>)` and `Outcome::Answer(Answer<A>)`; delete `checked_answer` and its test; add the "confidence and note kept" test. Verify `cargo test -p agent-loop` passes.

## 2. Jev client changes

- [x] 2.1 `clients::typesafe`: make `SystemOneRequest<S: Serialize = ResidualCallState>` generic and `system_one` generic over `S`; make `Question::choice` public. Verify `cargo build -p extractor-runtime` and `cargo test -p clients` pass unchanged.

## 3. Shared adapter types

- [x] 3.1 Add `clients`, `async-openai`, `reqwest` to `agent-loop/Cargo.toml`; add `src/adapters/mod.rs` with `AnswerOption<A>` and `Answers<A>`; export from `lib.rs`. Verify `cargo build -p agent-loop`.

## 4. Jev adapter

- [x] 4.1 `adapters/jev.rs`: `JevWording`, `JevLimits` (0.7 / 0.5), `JevState { task, facts }`, `questions()` asking `answer` always and `enough` + `next` only with options; >255 options -> `Err`. Verify tests: no options -> `answer` only; with options -> all three; 300 options -> error.
- [x] 4.2 `step_from()`: enough >= threshold, `stop`, missing `next` or no options -> answer; else `Call(next)`; answer label with probability >= 0.5 -> its value + confidence, else `none` value; label outside the set -> `Err`. Verify one test per spec scenario (not enough, enough, unsure, stop, outside set).
- [x] 4.3 `JevModel` implementing `Model<A>`: `id()` = `JEV_MODEL`; `step()` builds questions, calls `system_one`, maps errors to `Err(String)` without the key, adds usage tokens. Verify `cargo build -p agent-loop`; the missing-key case is `TypeSafeClient::new` returning `Err`, which `JevModel` needs before it can exist.

## 5. System-two adapter

- [x] 5.1 `adapters/openai.rs`: `system_two_client(base_url)` (key from `LLM_API_KEY`, fallback `ollama`, 120 s timeout); `request()` with system protocol message, user message (task, facts, options, answers), temperature 0, schema `{pick, note}` with enum `call:<id>` / `answer:<label>` / `none`. Verify tests: enum holds exactly the offered ids, labels and none; no options -> answer picks only.
- [x] 5.2 `reply_from()`: parse content JSON; `call:` -> `Call`, `answer:` -> `Final` with note, `none` -> `none` value; unknown pick or bad JSON -> `Err`; usage from `prompt_tokens` / `completion_tokens`. Verify one test per spec scenario, including tool id equal to a label.
- [x] 5.3 Context check in `reply_from()`: with `context_size`, `prompt_tokens >= size` or missing usage -> `Err` naming the overflow; without it, missing usage -> 0 tokens. Verify the three spec scenarios as tests.
- [x] 5.4 `OpenAiModel` implementing `Model<A>`: `id()` = model name; `step()` = `request()` + `create_byot` + `reply_from()`. Verify `cargo build -p agent-loop`.

## 6. Check

- [x] 6.1 Run `cargo test -p agent-loop -p agentic-models -p clients`, `cargo clippy -p agent-loop -p agentic-models -p clients -- -D warnings`, `cargo fmt --check`, `openspec validate agent-loop-adapters --strict`. Verify all pass; no function over ~40 lines, no non-test file over ~200 lines.
