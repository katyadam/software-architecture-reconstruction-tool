## Why
`matcher-jev` needs read-only tools that Jev can call per initiator. The architecture lists them: `similar`, `service_receivers`, `define`, `callers`, `receiver`, `constant`, `open_file`, `imports`. The code facts already exist in `extractor-runtime/src/pipeline/pass3/llm_enhance/facts`. They are private and tied to the DDFA residual: `FactCtx` holds a `RestCall`, its operand bindings and a `ResidualCallState`. `evaluate()` consumes the `ProjectIR`, so the agentic CLI loses it after extraction. Building the tools first lets `matcher-jev` stay about prompts and sessions.

## What Changes
- `evaluate()` borrows the `ProjectIR`. A sibling function also returns each REST call's operand bindings. The baseline output stays unchanged.
- The code facts move from `llm_enhance/facts` to a public `pipeline/facts` module. They work on a neutral `Subject`: origin file, enclosing function, receiver type, enclosing class, symbol names and mention text. A subject is built from a REST call with its bindings, or from a message edge. DDFA builds its subject from its prompt state, and a test checks that the REST call constructor gives the same subject. DDFA option ids and fact bodies stay byte-identical.
- New crate `agentic/tools` (`agentic-tools`) implements `agent_loop::Tools` for one initiator point:
  - code facts, through `pipeline/facts`
  - `similar`: the top receivers in other services, with the same protocol and, for HTTP, the same method, ranked by how close their address is to the initiator's
  - `service:<name>`: all receivers of one configured service with the initiator's protocol
  - `imports`: the files the origin file imports, and the files that import it
  - Every receiver row carries its point id. Every output is bounded in lines.
- `agentic-cli` output is unchanged. It starts keeping the `ProjectIR` and the bindings in `matcher-jev`, where the tools read them.
- No model call, no Ollama, no prompt. Jev sessions come in `matcher-jev`.

## Capabilities

### New Capabilities
- `agentic-tools`: the tool options offered for one initiator point, their ids, what each returns, and output bounds.

### Modified Capabilities
None. `agentic-static-matcher` output is unchanged. The DDFA facts have no main spec, and their behavior is preserved.

## Impact
- `extractor-runtime`:
  - `pipeline/pass3/mod.rs`: `evaluate` borrows the IR, and a new `evaluate_with_bindings` returns the bindings too
  - `llm_enhance/facts/` moves to `pipeline/facts/`, with `Subject` in place of `rc`, `bindings` and `state`
  - `llm_enhance/dispatch.rs` builds a `Subject`
  - test fixtures follow the move
- Callers of `evaluate`: `cli`, `extractor-runtime/src/api/service.rs`, `agentic/cli/src/extract.rs`, and the extractor-runtime, cli e2e and synthesizer tests. They pass `&ir`.
- New crate `agentic/tools`, depending on `agent-loop`, `agentic-models`, `extractor-runtime` and `models`. The workspace glob `agentic/*` picks it up.
