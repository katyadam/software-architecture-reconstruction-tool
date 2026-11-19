## Context

`agent-loop` has `Model<A>`, `Tools`, `run_session` and the JSONL log. No `Model` implementation exists.

- **Jev:** `clients::typesafe` has `TypeSafeClient::system_one` and question builders. `SystemOneRequest.state` is fixed to `ResidualCallState`, which only fits residual REST calls.
- **DDFA:** `extractor-runtime/.../llm_enhance/ddfa.rs` already runs the `enough` / `next` / `target` loop on Jev. This change ports its decision rule, not its code.
- **sage:** `sage/src/resolver/client.rs` shows the OpenAI-compatible pattern: `async-openai` with a JSON-schema `response_format`, `strict: false` (Ollama rejects strict with nullable enums), and an untyped `create_byot` reply (Groq adds fields `async-openai` rejects).

See `proposal.md` for why citations go.

## Goals / Non-Goals

**Goals:**
- Two `Model` implementations a stage can build per session.
- Request building and reply parsing as pure functions, so tests need no HTTP.
- Citations removed end to end: `agent-loop` and `agentic-models`.

**Non-Goals:**
- Stage prompts, question wording and answer sets. Those belong to `matcher-http` and later stages.
- Retries and backoff. `async-openai` already retries rate limits; Jev keeps its existing `ponytail:` note.
- Native tool calling.
- Running Ollama or any live model in tests.

## Decisions

### One answer type for step and outcome
```rust
pub struct Answer<A> { pub value: A, pub confidence: Option<f64>, pub note: Option<String> }
Step::Final(Answer<A>)
Outcome::Answer(Answer<A>)
```
- `Jev` fills `confidence`; system two fills `note`.
- The session passes it through unchanged.
- Alternative: separate fields on both enums. Rejected: two copies of the same three fields.

### Answer options given per session
```rust
pub struct AnswerOption<A> { pub label: String, pub description: String, pub value: A }
pub struct Answers<A> { pub options: Vec<AnswerOption<A>>, pub none: A }
```
- The stage builds the adapter per session, with that session's answers.
- The matcher builds a fixed endpoint set from `service_receivers` (decided; specified in `matcher-http`).
- `A: Clone`, because the picked value is cloned out of the options.
- Alternative: put the answers in `Request`. Rejected: the answers never change within a session, and `Request` stays provider-neutral.

### Jev adapter
- **Struct:** `JevModel<'a, A> { client: &'a TypeSafeClient, wording: &'a JevWording, answers: Answers<A>, limits: JevLimits }`.
- **Wording:** `JevWording` holds the question texts (`answer`, `enough` with yes/no criteria, `next`). It is stage-specific, so it has no default.
- **Limits:** `JevLimits { enough: 0.7, answer_min: 0.5 }`. The values come from DDFA's `Limits`.
- **State:** `JevState { task: String, facts: Vec<Fact> }`.
  - `SystemOneRequest<S: Serialize = ResidualCallState>` becomes generic.
  - The default type parameter keeps `ddfa.rs`, `dispatch.rs` and `residual_edge_filter.rs` compiling unchanged.
- **Question keys:** `answer`, `enough`, `next`, with options `none` and `stop`.
  - The adapter defines its own key constants.
  - It reuses `Question::choice` (made `pub`) for the 255-option check.
- **Pure functions:**
  - `questions(wording, answers, options) -> Result<BTreeMap<..>, String>`
  - `step_from(&SystemOneResponse, answers, limits, options_offered) -> Result<Step<A>, String>`
- **`step` method:** builds the questions, calls `system_one`, and maps any `TypeSafeError` to `Err(String)`.
- **Model id:** `JEV_MODEL#<12 hex>`, a hash of wording, limits and answers. Fields are private and `new()` computes the id, so editing any setting changes the prompt hash and skips stale log replays.
- **Option cap:** `next` keeps the first 254 options (best first) plus `stop`. A reserved (`none`, `stop`) or duplicate id is an error, since Jev keys choices by id.
- **Unsure pick:** the `none` value, with `below <min>: <label> <p>` in the note, so thresholds can be retuned from logs.
- **Picks outside the set:** a picked answer label not in `answers` is an error. Jev can't return one, but the check costs one line.

### System-two adapter
- **Struct:** `OpenAiModel<'a, A> { client: &'a Client<OpenAIConfig>, model: String, answers: Answers<A>, context_size: Option<u32> }`.
- **Client:** shared across sessions. It is built once by `system_two_client(base_url)`:
  - key from `LLM_API_KEY`, falling back to `ollama`, as in sage
  - a 120 s `reqwest` timeout
- **Pick ids are prefixed:** `call:<option id>`, `answer:<label>` and `none`.
  - The prefix makes a tool id and an answer label that are equal still map unambiguously (spec scenario).
  - Alternative: reject colliding ids. Rejected: it pushes the problem onto the stage.
- **Schema:** `{pick: enum, note: string}`, both required, with `strict: false` as in sage. Membership is checked in code anyway, so a pick outside the set is a model error.
- **Messages:**
  - A fixed system message explains the protocol: call a tool to fetch a fact, or answer.
  - The user message renders the task, the facts (`f1 [tool args]` plus text), the tool options with descriptions, and the answer options with descriptions.
- **Sampling:** temperature 0, for repeatable runs.
- **Pure functions:**
  - `request(task, facts, options, answers, model) -> CreateChatCompletionRequest`
  - `reply_from(&serde_json::Value, answers, options, context_size) -> Result<Reply<A>, String>`
- **Context check:** `prompt_tokens >= context_size` fails the step. Ollama's OpenAI endpoint ignores `num_ctx`, so the context is set server-side (`OLLAMA_CONTEXT_LENGTH`), and the same number is passed here.
  - Alternative: count tokens before sending. Rejected: it needs a tokenizer per model.
- **Model id:** `<model>#<12 hex>`, a hash of the system prompt and answers, computed in `new()`.
- **Calls:** any `call:` pick is returned as a call. The loop rejects unoffered ids and ends a spent budget as exhausted.
- **Fenced JSON:** not stripped. Whether real system-two models wrap replies in fences is checked in the verifier change.

### Layout
```
agent-loop/src/
  adapters/
    mod.rs        Answer options, shared
    jev/          mod.rs JevModel + types, questions.rs, reply.rs
    openai/       mod.rs OpenAiModel + client, request.rs, reply.rs (picks, context check)
    tests/        jev.rs, openai.rs
```
`openai` passed 200 lines, so it is a folder. Functions stay short.

### Evidence as `Vec<Fact>`
- **models:** `agentic-models` drops `Evidence` and `Claim`. `Link`, `Unmatched`, `Excluded` and `Verdict` hold `evidence: Vec<Fact>`.
- **Verifier rule:** the rule "a drop must cite facts from its own session" becomes "a drop needs at least one fact from its own session". That rule is the verifier's, and lands in its change.

## Risks / Trade-offs

- [Jev was tuned on a structured `ResidualCallState`; a free-text `task` may score worse] -> The matcher may render the task as compact JSON. Compare against the DDFA baseline in `matcher-http`.
- [Ollama may truncate to fewer than `context_size` tokens, so `>=` misses it] -> Verify once against a real Ollama with an oversized prompt, when the user allows running it. If needed, add a margin (for example 95%) as a constant.
- [The pure-function split leaves the HTTP glue untested] -> The glue is about 10 lines per adapter. It is covered later by the first `matcher-http` run.
- [Old JSONL logs no longer parse] -> None exist yet. A malformed line is a miss already.
- [Removing `Claim` loses which claim a fact backs] -> Never filled by the model anyway. Add per-claim citations to the system-two schema if research needs them.

## Migration Plan

No deployed users. Delete any local `agent-loop` logs before the first run after this change.

## Open Questions

- Default Jev wording for the matcher's service and endpoint questions: reuse `clients::typesafe::question::resolve` texts or write new ones. Decided in `matcher-http`.
