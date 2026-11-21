## Why
The static matcher links every candidate it finds. Ties stay as false positives, and calls with no candidate are lost. The baseline only gets recall back with `fuzzy_fallback`, a Levenshtein guess with no evidence. The tools from `facts-tools` and the loop from `agent-loop` are ready. This change lets Jev decide every call the static rules cannot settle, and every decision carries the facts it was based on.

## What Changes
- **BREAKING (score):** `fuzzy_fallback` and `DISSIMILARITY_PERCENT` are removed. A call with no unified candidate now has no candidate. Without `--jev`, empaia drops from 0.941 to 0.909 F1 (`app-service -> medical-data-service` is lost). With `http://mds` added to medical-data-service in the empaia configs, static gives 0.938 (P 1.000, R 0.882), and the parity requirement moves there.
- A static link now carries one runtime fact: the rule that found it (`exact`, `template` or `suffix`) and the receiver address.
- New flag `--jev` on `agentic-cli`. Without it, output is the same as today minus fuzzy matches. With it:
  - A **strong match** stays `Static`: exactly one candidate, tagged `Exact` or `Template`.
  - Every other HTTP initiator goes to Jev. That covers no candidate, several candidates, and a single `Suffix` candidate.
  - Jev runs two sessions per initiator through `agent-loop`, with `agentic-tools` as its tools. First it picks a service, then an endpoint of that service. Both answers come from a closed set; endpoints are picked by point id.
  - Outcomes:
    - Endpoint picked -> `Jev` link to it.
    - Service picked but no endpoint -> `Jev` link with no receiver.
    - No service picked -> `Unmatched`, with reason `NoFit`, `BudgetExhausted` or `ModelError`.
    - The link or `Unmatched` carries the facts of both sessions as evidence.
  - Sessions are logged to `match/sessions.jsonl`, so a rerun replays them.
- `agentic-cli` keeps the `ProjectIR` and the operand bindings after extraction. Bindings are keyed by point id.
- `metrics.json` gains Jev counts: sessions, model calls, tokens, replayed sessions, and outcomes.
- Message protocols are unchanged. They still have no rules and no Jev.

## Capabilities

### New Capabilities
- `agentic-jev-matcher`: which initiators go to Jev, the two sessions and their answer sets, outcomes, evidence, session log and Jev metrics.

### Modified Capabilities
- `agentic-static-matcher`:
  - no fuzzy fallback
  - static links carry one rule fact
  - parity is 0.938 with the updated empaia config
  - the `--jev` flag lifts the "no model" requirement

## Impact
- `agentic/matcher`:
  - `http/mod.rs` loses the fuzzy fallback
  - static links get their fact
  - new `jev/` module: selection, task text, answer sets, outcomes
  - new dependencies: `agent-loop`, `agentic-tools`, `extractor-runtime`
- `agentic/cli`:
  - `extract.rs` returns the IR and the bindings by point id
  - the `--jev` flag, a Jev client from `TYPESAFE_API_KEY`, and a tokio runtime
  - the session log is written under `match/`
- Root `Cargo.toml`: `agentic-tools` and `agent-loop` become workspace dependencies, if they are not already.
- The baseline `cli`, the synthesizer and DDFA are unchanged.
- A run with `--jev` calls the TypeSafe API and needs a key. No local model is started.
