## Why

The agentic SDG has models and an agent loop, but no stage that produces `Matches` from a real project. A static-only matcher is the first stage that runs end to end. It must reproduce the static baseline score, so later Jev stages are measured against a known start. It also counts candidate fan-out, which sizes the Jev work before it is built.

## What Changes

- New `agentic/cli` crate, binary `agentic-cli`. Flags: project dir, config, constants file, output dir. Extracts with passes 1–3, no LLM, no Ollama.
- New `agentic/matcher` crate. REST only.
  - Builds `ConnectionPoint`s from evaluated REST calls and endpoints.
  - Assigns services by path segment (shared with synthesizer in `models`). Unassigned points are logged and counted. Unassigned calls are not matched; unassigned endpoints stay candidates under the empty service.
  - Substitutes constants into call targets (shared with synthesizer in `models::substitute`). `element` stays unchanged.
  - Finds candidates with a copy of `synthesizer/src/sdg/matching.rs::candidates()`.
  - Every candidate becomes a `Link` with `MatchSource::Static`, as `builder.rs:302` does.
  - No candidate -> `Unmatched` with `NoCandidate`.
  - Kind per link from `classify()`, rolled up per `Dependency`.
  - Fan-out metrics per initiator: empty, single, same-service tie, cross-service tie.
- `classify()`, `is_reflexive()`, `host_of()` move from `synthesizer/src/sdg/interaction_kind.rs` to `models`. Synthesizer re-exports them. One copy, so kinds cannot drift.
- Outputs: `match/sdg.json`, `match/sdg.legacy.json`, `match/unmatched.json`, `match/metrics.json`, plus `final/sdg.json` and `final/sdg.legacy.json`.
- Not in scope: message edges (empaia has none), Jev, triage, tools, `--scrape`, WALA.

## Capabilities

### New Capabilities
- `agentic-static-matcher`: static matching of REST points into `Matches`, its outputs, metrics and the baseline parity check.

### Modified Capabilities
None.

## Impact

- New crates: `agentic/cli`, `agentic/matcher`. Workspace glob `agentic/*` picks them up.
- `models`: gains the interaction-kind rules.
- `synthesizer`: `interaction_kind.rs` shrinks to a re-export. No behaviour change; its tests must stay green.
- Baseline `cli` untouched.
- Acceptance: empaia via score.py = P 0.941, R 0.941, F1 0.941.
