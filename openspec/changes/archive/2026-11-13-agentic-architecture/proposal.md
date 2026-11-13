## Why

The agentic SDG design lives only in two claude.ai artifacts and chat history. Code has started (`agentic-models`, commit `6e416e3`) without a spec. Later changes (`agent-loop`, `matcher-http`, `verifier`, `rematcher`) need one recorded architecture and one contract for the shared `Matches` types.

## What Changes

- Record the agentic SDG architecture in `design.md`: stages, crates, data flow, tools, evidence model, outputs, evaluation arms.
- Record decisions made after the artifacts:
  - `Matches.excluded` holds points left out on purpose (hole 4).
  - `UnmatchedReason::ModelError` separates provider failure from model decision (hole 5).
  - Protocol comes from the code element; gRPC `MessageEdge` maps to `Grpc` (hole 8).
  - Tool protocol: one schema-enforced JSON choice per step. No native tool calling. No separate spike (hole 6).
  - Evidence cites runtime-recorded facts, not file and line (holes 1, 2).
  - `PointId` is a readable string, not a hash.
  - `InteractionKind` lives in root `models`; synthesizer re-exports it.
  - Scope cut: HTTP first. Channel types exist but stay unmatched until an oracle covers them.
- Spec the already-built `agentic-models` contract as capability `agentic-matches-model`.
- Fix the follow-up change order.

No code changes. Built code is checked against the new spec.

## Capabilities

### New Capabilities
- `agentic-matches-model`: shared output types of the agentic stages: `Matches`, SDG, links, connection points, unmatched and excluded points, evidence, stable point ids, protocol mapping.

### Modified Capabilities
None.

## Impact

- New: `openspec/changes/agentic-architecture/` and, on archive, `openspec/specs/agentic-matches-model/spec.md`.
- Describes existing code: `agentic/models/src/lib.rs`, `models/src/interaction_kind.rs`.
- Guides follow-up changes, in order:
  1. `agentic-models-verdicts`: `Verdict`, `Decision`, `LinkId`, `ConnectionPoint::location()`, `"agentic/*"` members glob.
  2. `agent-loop`: generic loop, JSON choice per step, facts, budget, JSONL call log and resume.
  3. `matcher-http`: points, service index, `match_point`, `Outcome`, `Prior`, `Matches` and legacy `sdg.json`.
  4. `verifier`, then `rematcher`.
- Baseline `cli` stays frozen. `synthesizer` is not used by the agentic path.
