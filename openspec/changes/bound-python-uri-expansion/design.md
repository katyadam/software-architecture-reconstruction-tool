# Design

## Context

`generate_target_uris` folds the resolved parts into URIs with nested loops, materialising `acc.len() * parts.len()` strings at every step. Nothing bounds `acc`.

## Goals / Non-Goals

**Goals:**
- Never hold more than the cap of URIs at any fold step, so memory stays bounded.
- Deterministic output past the cap.

**Non-Goals:**
- Making the cap configurable, or warning when it is hit.
- Bounding the Java extractor's identical fold (separate crate, separate issue).

## Decisions

- Cap value `MAX_TARGET_URIS = 256` as a module constant. Large enough for any realistic endpoint fan-out, small enough to be cheap.
- Each fold step becomes a lazy `flat_map` over `(base, part)` followed by `.take(MAX_TARGET_URIS)`. Because `acc` is already capped, no step builds more than the cap. Alternative: build the full product and truncate at the end; rejected because the intermediate vectors are exactly what exhausts memory.
- Keeping the first N in product order gives determinism for free (part values come in their resolved order). Alternative: sorting before truncating; rejected as extra work and it would still need the full product.

## Risks / Trade-offs

- [Over-cap targets lose later combinations, so some real endpoints may not be matched] → Acceptable: such targets are pathological, and partial output beats a crashed run.
- [Past the cap only the trailing parts vary, so all kept URIs share the first value of the leading parts (e.g. one service out of several)] → Accepted for now; a fairer spread can follow if real targets hit the cap.
- [Hitting the cap is silent, and duplicate combinations count towards it] → Out of scope for this minimal fix; candidates for a follow-up.
