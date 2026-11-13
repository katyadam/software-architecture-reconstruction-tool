## Why

Matcher, verifier and rematcher all run the same loop: ask the model, fetch a fact, ask again, stop. Today that loop lives only in `extractor-runtime` DDFA, tied to Jev and to REST residuals. The agentic stages need one model-agnostic loop with a budget, cited facts, and a crash-safe log before any stage can be built.

## What Changes

- New crate `agentic/agent-loop` (package `agent-loop`).
- A `Model` trait: one call returns one `Step`, either a tool call or a final answer. Adapters per provider come later.
- A `Tools` trait: lists the tool options still open and runs one. Stages implement it.
- `run_session`: the generic loop. Ports DDFA's rules: step budget, taken tools not offered again, a last tool-free ask when the budget runs out.
- The runtime records each tool result as an `agentic_models::Fact` with a numbered id (`f1`, `f2`, ...). A final answer may cite only those ids.
- Provider failures and invalid replies end the session as `ModelError`, not as a model decision.
- A JSONL session log: one record per finished session, with stage, point id, prompt hash, facts, answer and tokens. A rerun with the same stage, point and prompt hash replays the logged answer without a model call.
- Out of scope, and moved to `agent-loop-adapters`: the Jev `choice` adapter, the system-two JSON-schema adapter, context size and the token-mismatch check.
- Out of scope, and moved to `matcher-http`: `Context` with `ProjectIR`, and exposing the `llm_enhance/facts` tools. Nothing in this change consumes them yet.

## Capabilities

### New Capabilities
- `agent-loop`: the generic model loop. Covers step protocol, budget, fact numbering and citation, error handling, session log and resume.

### Modified Capabilities

None.

## Impact

- New crate `agentic/agent-loop`. Dependencies: `agentic-models`, `serde`, `serde_json`, `sha2`. `tokio` is a dev-dependency only, for async tests.
- The root workspace already picks the crate up via `agentic/*`. Add `agent-loop` to `[workspace.dependencies]`.
- No change to `extractor-runtime`, `clients` or the existing CLI. DDFA stays as it is.
- No model is called in tests. A scripted fake model drives every scenario.
