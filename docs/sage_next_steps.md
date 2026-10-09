# VOYANTCLAIR — Next Steps

Prioritized work after the sage-rework Phase 3 completion and the 23.07.2026
agenda. Ordered by leverage. Companion docs: `sage_rework_results.md` (durable
measurement log), `sage_rework_knowledge_report.md` (paper-facing),
`superpowers/specs/2026-07-21-sdg-precision-typed-edges-design.md` (the A/B/C
design).

Baseline best results (business-edge P/R vs hand-verified oracle):
Measured 2026-10-09 at `4fb2a11` (`sage-rework`), Business view; details in
`sage_rework_results.md` "Jev triage — end-to-end SDG edges".
- empaia, constants + scrape, matcher + Jev triage (`--llm --typesafe`,
  Sage unreachable): **0.88 / 0.88** median of 6 (FP 1–2). Same without
  Jev: 0.69 / 0.84 (FP 6). No `--llm`: 0.86 / 0.75.
  **Nondeterministic** (R spread 0.69–0.94 across runs): external-constants
  `HashMap` tie, see that entry. Report medians of >= 5 runs.
- train-ticket Java (any arm): **1.00 / 0.95** (87 TP, 0 FP, 5 FN).
- train-ticket polyglot, scrape, no Sage (any arm): **0.98 / 0.90**
  (83 TP, 2 FP, 9 FN).
- Not re-measured (Ollama not installed): any live-Sage run. The old empaia
  LLM 0.74 / 0.88 and polyglot + LLM 0.86 / 0.77 are stale; do not compare.
- Micrograal baseline (old tree, not re-measured): 0.68 / 0.59.

---

## 1. Category D — remaining empaia FPs — DO FIRST (promoted 2026-07-31)

Deferred no longer — A/B/C landed, this is next. All constant/host -> service
misresolution (not classification). The current no-LLM run (typed-edges
branch, though the typed-interaction change is a no-op on empaia — see item
2) confirms 2 of the original 3 are still live: `annotation-service ->
clinical-data-service`
(`slide_info_url`, should be mds) and `workbench-service -> app-service`
(`frontends.py`/`data.py` host mismatch). `marketplace-service -> app-service`
(`vault_client.py`) is unconfirmed — it does not appear in the no-LLM run's
2 remaining FPs, but the LLM-run baseline above has not been re-measured, so
do not assume it is resolved there. Per-case resolver work; lifts empaia
precision past the current no-LLM baseline (0.87).

## 2. Ship typed-edge change (A/B/C) — DONE (2026-07-31)

Classified each SDG connection as Business / HealthInfra / Reflexive /
TestOrigin; only Business connections are scored. Pure synthesizer change
(`synthesizer/src/sdg/`), no extraction touched.

- **A — test-origin tagging:** marks requests whose `file_path` is test code
  (Java `/src/test/`, `*Test.java`; Python `test_*.py`, `conftest.py`).
- **B — health-infra typing:** marks `/alive`, `/health`, `/actuator/*`, etc.
- **C — reflexive rewrite:** `localhost`/`127.0.0.1`/own-config-host -> self-loop
  edge instead of matching another service.
- **Scoring:** `ground_sdg.py` / `ground_empaia.py` count only `kind==Business`;
  legacy SDGs default to Business (backward compatible).
- **Measured — train-ticket, all-Java, no LLM** (`results/train-ticket-java-typed`),
  vs the 92-edge oracle: precision **0.98 -> 1.00**, recall 0.95 -> 0.95
  (87 TP, 2 -> **0** FP, 5 FN). 89 connections: 87 `Business` + 2 `TestOrigin`
  (`ts-preserve-service -> ts-notification-service`,
  `ts-preserve-other-service -> ts-notification-service`) — exactly the two
  former false positives.
- **Measured — empaia, no LLM, constants + scrape — is a no-op (corrected
  2026-07-31).** The originally published 0.71 -> 0.87 comparison used
  `results/empaia-nollm-baseline`, built 2026-07-19 at commit `168f757` from
  a dirty tree with only 405 restcalls — not this branch's actual parent, and
  invalid as a baseline. Retracted.

  A controlled A/B (parent `cb33e56` in a worktree vs this branch's `6097d4f`,
  identical flags) shows **identical** results: 0.87 / 0.81, 13 TP / 2 FP /
  3 FN, 15 connections, 444 requests, on both sides. Every one of the 444
  requests classifies `Business` — nothing is reflexive, test-origin, or
  health, so there is nothing for the classifier to reclassify. With
  constants substituted, empaia's localhost URLs already resolve to real
  hostnames before matching runs.

  The real 0.71 -> 0.87 / 0.75 -> 0.81 jump is genuine, but it happened in
  earlier sage-rework commits between `168f757` and `cb33e56` — not from
  Component A/B/C. Full numbers: `sage_rework_results.md` S4.1.
- **Outstanding:** the empaia LLM + constants + scrape re-run has not been
  done — blocked on Ollama not running. The published 0.74 / 0.88 headline
  above is from that run; do not conflate it with the no-LLM figure.

## 3. Hand-labeled ground-truth pass (paper blocker)

Auto-oracle is 0-scoreable on the current residual tail -> GATE C is only
soft-passed. Post extraction-fix the residual set is tiny (~19 empaia + ~2
train-ticket). Hand-label all of them -> real precision/recall per tier.

## 4. Polyglot gaps (the 0.86 / 0.77 story)

- **Go extractor** — Go-blindness (route-plan / station have no extractor) drove
  most of the 21 misses. Biggest recall lever.
- **Tighten LLM `validate()`** — target-substitution added most of the 12 FPs;
  require the chosen service to be supported by the cited evidence (the deferred
  grounded ≠ correct fix; `ts-price` -> `ts-consign-price` failure mode).

## 5. Paper hardening (lower urgency)

- Contamination control: anonymize `ts-*` names (train-ticket likely in the
  model's training data); report plain vs anonymized.
- Nondeterminism: temperature 0 + fixed seed, N≥5 runs, report variance.
- Ablation table: old gate vs structural gate vs +hygiene vs +matcher vs +LLM.
- End-to-end SDG-edge P/R vs prior SAR literature; June 0%-precision run is a
  built-in baseline.
