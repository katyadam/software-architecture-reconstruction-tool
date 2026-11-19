## Why

`agent-loop` defines the `Model` trait, but nothing implements it. No stage can talk to a real model yet. The matcher needs Jev, and the verifier and rematcher need a system-two model. Citations also add nothing. Jev cannot pick them, so a Jev answer would just cite every fact. The session's facts already hold the same information.

## What Changes

- **Jev adapter**: implements `Model` over the TypeSafe System One API. It ports the DDFA step from `extractor-runtime`: each call asks `enough` (noul), `next` (tool options + `stop`) and the answer question (answer options + `none`). It calls the chosen tool, or answers.
- **System-two adapter**: implements `Model` over any OpenAI-compatible endpoint, with a JSON-schema response format. Each reply picks one id from a closed enum (tool options + answer options) plus a short note.
- **Closed-set answers**: both adapters take the stage's answer options, each a label, a description and a value. The model only ever picks a label.
- **Context overflow check**, system two only: the context size is configured. A reply whose prompt tokens reach it fails as a model error (Ollama truncates silently).
- **BREAKING** – citations removed:
  - `Step::Final` loses `cites` and gains an optional `confidence` (from Jev) and an optional `note` (from system two)
  - `Outcome::Answer` changes the same way
  - the "Answers cite only recorded facts" requirement goes
- **BREAKING** – evidence is a fact list: `Evidence { claim, fact }` and `Claim` are removed from `agentic-models`. Links, unmatched points, excluded points and verdicts hold `evidence: Vec<Fact>`.
- `clients::typesafe`:
  - `SystemOneRequest` becomes generic over its state, so the adapter can send `{task, facts}`
  - `Question::choice` becomes public, so the adapter reuses its 255-option check

## Capabilities

### New Capabilities
None. The adapters belong to `agent-loop`.

### Modified Capabilities
- `agent-loop`:
  - the final step carries an answer, optional confidence and optional note, with no citations
  - citation check removed
  - new requirements: closed-set answers, the Jev adapter, the system-two adapter, the context overflow check
- `agentic-matches-model`: evidence becomes the list of facts a session recorded. Claim pairing is removed.

## Impact

- `agentic/agent-loop`:
  - new module `src/adapters/` (`jev.rs`, `openai.rs`)
  - `protocol.rs` and `session.rs` lose citations
  - log lines change shape, so old logs no longer replay. None exist yet.
- `agentic/models`: `Evidence` and `Claim` removed, fields become `Vec<Fact>`.
- `clients/src/typesafe`: generic request state, public `Question::choice`. Existing DDFA callers keep compiling through a default type parameter.
- New dependencies of `agent-loop`: `clients`, `async-openai`, `reqwest`. All are already in the workspace.
- No live model runs in tests. Request building and reply parsing are pure functions, tested without HTTP.
