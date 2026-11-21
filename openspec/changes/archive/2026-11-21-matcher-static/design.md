## Context

- The static baseline runs `cli`: extract -> `CodeElementsAggregate` -> `synthesizer::direct_sdg_build_with_unmatched`. Synthesis does three things before pairing:
  1. assigns services (`utils.rs:3`)
  2. substitutes constants into call targets (`builder.rs`)
  3. builds `all_urls` from config.
- `create_endpoint_restcall_pairs` (`builder.rs:302`) keeps every candidate. The `.941` score depends on that.
- `classify()` lives in `synthesizer/src/sdg/interaction_kind.rs`. It needs only `models` types.
- score.py reads `connections[].{source_id, target_id, kind, requests[].restcall.file_path}`. It skips empty source or target and non-`Business` connections.
- The 2026-11-13 baseline (`results/bench-41b9691-dirty/E-static`) used `local-empaia-config.json`, `empaia-constants.json`, no scrape, no LLM.

## Goals / Non-Goals

**Goals:**
- Same service edges as the baseline on empaia.
- Code split so `matcher-jev` replaces only the "every candidate is a link" step.

**Non-Goals:**
- Keeping `ProjectIR` after extraction. `facts-tools` needs it; that change makes `evaluate()` borrow.
- A shared candidate function with synthesizer. Copied, free to diverge (architecture design).
- `--scrape`, WALA, IMCG, context map.

## Decisions

### Crates and modules
```
agentic/cli/        agentic-cli (bin)  -> matcher, extractor-runtime, models
  main.rs           args, run order
  extract.rs        passes 1-3 -> EvaluatedIR
  output.rs         write match/ and final/
  legacy.rs         Matches -> synthesizer JSON shape
agentic/matcher/    agentic-matcher     -> agentic-models, models
  lib.rs            match_static(endpoints, calls, config, constants) -> (Matches, Metrics)
  points.rs         Placed points, ConnectionPoint build
  candidates/
    mod.rs          candidates(), scope() (copied)
    segments.rs     match_tag, unify, fuzzy, drop rules (copied)
  group.rs          links -> dependencies, kind, rollup
  metrics.rs        fan-out counts
```
Matcher takes plain element slices, not extractor types, so it builds without extractor-runtime. That keeps its tests quick.

Alternative: depend on the `cli` lib for extraction. Rejected: it pulls synthesizer, and slice 2 needs control over `ProjectIR`. The extraction is about 25 lines on `extractor_runtime::pipeline`. `collect_files` and `collect_syntactic_records` move from `cli` to `extractor_runtime::pipeline::files`, shared by both CLIs. `collect_files` sorts its output, so file order does not depend on the filesystem.

### Matcher-internal view of a point
`candidates()` needs the call's substituted target, method, and its service's urls. Inside the matcher, each point is held as `Placed { point: ConnectionPoint, service: &ServiceDescription, target: String }`. The copied `candidates()` reads `Placed` instead of `AssignedRestCall`/`AssignedEndpoint`. Its body stays the same apart from field paths.

`Address::Http { method, host, path }` is filled from the substituted target: `host = host_of(target)` when that is non-empty, and `path` = the rest. The original element keeps the raw target.

### Classification moves to `models`
`interaction_kind.rs` moves whole to `models/src/interaction_kind.rs`, next to `InteractionKind`: `classify`, `InteractionSignals`, `is_reflexive`, `host_of`, helpers. The synthesizer module becomes `pub use models::interaction_kind::*;`, so its imports and tests are unchanged. `request_kind()` (`builder.rs:316`, 20 lines) is copied into `group.rs`, since it is a thin call to `classify`.

Alternative: copy `classify` as well. Rejected: the architecture design requires equal semantics. Two copies would drift silently, and score.py filters on kind.

### Unassigned points
`models::configuration::assign_service` returns `Option`. The synthesizer wraps it with `unwrap_or_default`. `None` -> log to stderr and count. A call is then skipped. An endpoint stays a candidate under the empty service, as in the baseline, since it can dominate or win fuzzy matching. `Substituter` moves to `models::substitute`; the builder calls it.

Baseline difference: in the baseline, unassigned endpoints stay candidates and could dominate others. If parity fails, check this first.

### Legacy writer
A local serde struct mirroring synthesizer's `Sdg`/`Connection`/`Request`: `services`, `connections`, and `message_connections: []`.
- `Request` has `endpoint`, `restcall` and `kind`. It has no `match_tag`; score.py does not read it.
- `endpoint` is `null` when the receiver is `None`. That does not happen in this slice.
- The original element goes into `restcall`, so test-path detection uses the real file.

### Determinism
`BTreeMap` keyed by `(source, target)` for grouping. Services sorted by name, links by point id. Pretty JSON.

## Risks / Trade-offs

- [Parity misses because synthesizer services come from a `HashMap` order] -> score.py uses sets, so order does not affect the score.
- [Unassigned endpoints change candidate sets] -> parity check. Fallback: keep unassigned receivers as candidates under an empty service, then drop their links.
- [`constants.rs:53` HashMap tie in extractor-runtime] -> the baseline was stable over 4 runs. Run twice and diff.
- [Moving `classify` breaks synthesizer] -> `cargo test -p synthesizer` must stay green, and the module path is unchanged via the re-export.

## Migration Plan

None. New crates. Synthesizer keeps its module path.
