# Sage Rework — Results Log

Durable record of measurements, gate verdicts, and decisions for the rework in
[`sage_rework_plan.md`](sage_rework_plan.md). This file is the **only memory
that survives context compaction** — per the §4 execution rule, every phase ends
with its results written here. If a result is not in this file, it did not
happen.

- **Branch:** `sage-rework`
- **Convention:** append under the relevant phase; date each entry; record the
  raw number, the gate verdict, and the decision taken.

---

## Gate thresholds (committed in S0.5)

> Fill in during Phase 0. Starting points are revisable, but once written they
> govern cold-session decisions.

- **GATE A** — _the lexical gate is the leak:_ TBD (expected: `Junk:non-empty`
  bucket is non-trivial).
- **GATE B** — _is the LLM needed:_ deterministic tier resolves ≥ 60% → LLM
  fallback-only; ≥ 90% → LLM deferrable. (TBD: confirm/adjust.)
- **GATE C** — _train-ticket verdict:_ precision < 0.7 → revisit candidate
  narrowing (§6.4) before declaring done. (TBD: confirm/adjust.)

---

## Phase 0 — Foundation + baseline

- **S0.1 — Signal extractor:** _pending._
- **S0.2 — Ground truth + scorer:** _pending._
- **S0.3 — Baseline buckets** (old lexical gate, provisional):
  - `Enough`: _pending_
  - `NeedsLLM`: _pending_
  - `Junk:empty`: _pending_
  - `Junk:non-empty-but-failed-lexical-gate` (silent recall hole): _pending_
- **S0.4 — Strong-vs-thin split:** _pending._
- **S0.5 — Gate thresholds committed:** see section above.
- **GATE A verdict:** _pending._

---

## Phase 1 — Structural gate

- **S1.4 — Post-gate buckets** (`NeedsResolution` vs old `NeedsLLM`): _pending._
- **Refreshed strong-vs-thin split** (real population): _pending._

---

## Phase 2 — Service matcher

- **M1 — Deterministic-only coverage** (empaia / train-ticket): _pending._
- **GATE B verdict** (is the LLM needed): _pending._
- **Post-LLM coverage** (after S2.7): _pending._

---

## Phase 3 — Final evaluation

- **Empaia** — precision / recall: _pending._
- **Train-ticket** — precision / recall: _pending._
- **GATE C verdict:** _pending._

---

## Decision log

> Chronological record of decisions taken and why. One line each.

- _(empty)_
