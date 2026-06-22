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

- **S0.1 — Signal extractor:** _done (2026-06-22)._ New
  `llm_enhance/signals.rs`: `CallSiteSignals { origin_service, client_class,
  imports, operand_identifiers, candidate_services }` via
  `extract(rc, project_ir, config)`. `&ProjectIR` threaded into the LLM path;
  temporary `SIGNALS:` instrumentation in `dispatch.rs` (to be removed once the
  matcher consumes signals). Green: `cargo test -p extractor-runtime` 48 + 21
  pass, clippy clean (2 pre-existing warnings only).
  - **Correction:** `project_ir.callable_map` is keyed by **mangled name, not
    `function_hash`** (`pass2::callables::build_project_global_callables`). The
    plan/§5 assumption was wrong. Class is recovered by scanning the owning
    file's `callables` for `metadata.hash == rc.function_hash`. See memory
    `callable_map_keying.md`.
  - **Empaia validation (128 residuals, all Python):**
    - `client_class` recovered: **48 / 128** (was 0 before the keying fix). The
      other 80 are FastAPI module-level route handlers (legitimately `Module`).
    - operand identifiers present: **127 / 128**.
    - Sample target_uris are bare path params (`case_id`, `class_id`,
      `collection_id`, `annotation_id`) — flag for S0.3: the residual set looks
      polluted with non-client / path-fragment targets; the old lexical gate's
      behavior here needs the bucket breakdown.
  - **Known gap (defer):** class resolved only via hash; no mangled-name
    fallback when `rc.function_hash` is empty (unlike `restcalls.rs`). 1/128 had
    no operand identifiers — quantify empty-hash rate in S0.3.
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

- **2026-06-22** — Ground truth cannot be hand-supplied by the user. Scorer
  oracle is auto-derived from `empaia-constants.json` (identifier -> URL) +
  `empaia-config.json` (URL -> service). Limitation: only scores residuals whose
  identifier appears in the curated file. Not circular — classifier resolves from
  names/classes, never from the constants' URL values.
- **2026-06-22** — Phase 0's label-free measurements (GATE A buckets,
  strong-vs-thin split) do not depend on the oracle above; only the precision/
  recall scorer (S0.2) does.
- **2026-06-22** — Execution: Phase 0 implemented via Code Writer subagent per
  project CLAUDE.md agentic pipeline.
