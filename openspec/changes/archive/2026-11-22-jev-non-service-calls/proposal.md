## Why

The match-service session has no answer for "this is not a service call". In the best empaia `--jev` run, 114 of 146 service sessions picked the caller's own service. A sample of 22 had no real self-call:

- 14 go through the service's own database layer
- 5 hit an in-process cache or dict
- 2 are library calls (Docker SDK, a dict lookup)
- 1 calls external infrastructure (HashiCorp Vault)

Those links are `Reflexive`, so the SDG drops them and the score hides the noise. They still swamp the links, the session log and later stages (verifier, rematch). Two own-service picks hid real cross-service calls (`workbench v2 es_connector` -> medical-data-service; the `alive.py` fan-out, #90). Issue #87.

## What Changes

- The match-service answer set gains two fixed answers next to the services:
  - `external`: a network call to a system outside the configured services, e.g. Vault, a Docker registry, an arbitrary URL.
  - `in-process`: no network call at all, e.g. a database layer, a cache or dict, an SDK object.
- The service question text says that a call through a database layer, cache or SDK object is not a service call. It also says that the caller's own service is right only for an HTTP call to its own endpoints.
- `external` makes the point excluded with reason `External`. `in-process` makes it excluded with a new reason, `InProcess`. Excluded points carry the service session's facts as evidence, and no endpoint session runs.
- `Matches.excluded` is filled from Jev and written to `match/excluded.json`.
- Jev metrics count excluded points by reason.
- `none` keeps its meaning: a service call, but no listed service fits or the facts don't show which one.

Non-goals:
- A triage gate before Jev (the legacy `IS_HTTP` filter). It stays the fallback if the answers don't help.
- Multi-target call sites (#90).
- Infrastructure nodes in the SDG.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `agentic-jev-matcher`: the service session answer set, the question wording and the outcome table gain the two non-service answers. Metrics count excluded points.
- `agentic-matches-model`: `ExcludedReason` gains `InProcess`.
- `agentic-static-matcher`: the output files gain `match/excluded.json`.

## Impact

- `agentic/models/src/lib.rs`: the `ExcludedReason::InProcess` variant.
- `agentic/matcher/src/jev/`: `wording.rs` (the `Pick` type and question text), `answers.rs` (the two fixed answers), `outcome.rs` (the `Excluded` outcome), `mod.rs` (`JevResult.excluded`, metrics).
- `agentic/matcher/src/plan.rs`: `finish` takes the excluded points.
- `agentic/matcher/src/metrics.rs`: the `excluded` count.
- `agentic/cli/src/`: passes the excluded points along and writes `match/excluded.json`.
- The prompts change, so the logged sessions won't replay. The next `--jev` run on empaia is fresh: about 5 min and $0.24.
- No change to the baseline `cli`, DDFA or the extractors.
