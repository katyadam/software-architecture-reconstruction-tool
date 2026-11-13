## Why

The verifier returns verdicts, and the CLI saves them to `final/verdicts.json`. The shared crate has no `Verdict` type and no way to name a link. Later stages also need a point's location to show code. Settle these now, before `agent-loop` and `matcher-http` depend on the crate.

## What Changes

- Add `Verdict { link, decision, reason, evidence }` and `Decision { Keep, Drop, Unverified }` to `agentic-models`.
- Add `LinkId`: initiator point id plus either receiver point id or target service id. Derived from a dependency and one of its links.
- Add `ConnectionPoint::location()`: file path, plus byte span when the element has one (REST calls only).
- Switch workspace members from `"agentic/models"` to the glob `"agentic/*"`.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `agentic-matches-model`: adds verdicts, link ids and point locations.

## Impact

- `agentic/models/src/lib.rs`: new types and two methods, with tests.
- Root `Cargo.toml`: members glob. Every folder under `agentic/` must then be a crate.
- No change to baseline `cli` or synthesizer.
