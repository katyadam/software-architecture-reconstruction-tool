# Demand-Driven Fact Acquisition (DDFA) — Design

Date: 2026-10-11. Branch: `sage-rework`. Snapshot of the replaced Sage
resolver: tag `sage-llm-snapshot` (branch `sage-llm`).

## Problem

Residual REST calls (target URL not resolved by symbolic evaluation) pass
Jev triage (`is_http >= 0.8`) and the deterministic matcher. Matcher misses
go to Sage, a system-2 LLM. On empaia Sage adds nothing (SJ Groq
gpt-oss-120b: P .875 R .875 = DJ) and costs a rate-limited LLM. Sage sees a
fixed context; it cannot ask for the fact it lacks.

## Goal

Replace Sage with TypeSafe Jev (system 1, `jev-1.13.0`). Jev acquires
program-analysis facts on demand, then picks the **target service** (not a
URL). Output feeds the existing `rewrite_target_uri_to_service`.

## Name and literature

**Demand-Driven Fact Acquisition (DDFA).**

- Active Feature Acquisition: Saar-Tsechansky, Melville, Provost,
  *Active Feature-Value Acquisition*, Management Science 55(4), 2009;
  Shim, Hwang, Yang, NeurIPS 2018; Janisch, Pevný, Lisý,
  *Classification with Costly Features Using Deep RL*, AAAI 2019.
  A classifier decides: predict now, or acquire another feature.
- Demand-driven analysis: Duesterwald, Gupta, Soffa, TOPLAS 19(6), 1997;
  Heintze, Tardieu, PLDI 2001 (verify DOI before citing).
- Contrast: ReAct (ICLR 2023), IRCoT (ACL 2023), FLARE (EMNLP 2023),
  RepoCoder (EMNLP 2023) need a generative model. DDFA only picks closed
  options. The action set grows while exploring (`OpenFile` hops).
  No RL policy; Jev picks zero-shot.

## Architecture

```
residual
  -> Jev triage (is_http >= 0.8)   unchanged
  -> deterministic matcher         unchanged
  -> matcher miss -> resolver      Matcher | Sage | DDFA(once|acquire)
  -> synthesizer
```

Jev answers only Noul / Choice (<= 255 options) / Score. DDFA asks:

| Id | Type | Options |
| --- | --- | --- |
| `target` | Choice | candidate services + `none` |
| `enough` | Noul | facts identify the target? |
| `next` | Choice | available actions + `stop` |

All three go in **one** request per step. `enough`/`next` are omitted when no
action is available.

### Actions (facts)

| Action | Source | Fact |
| --- | --- | --- |
| `Define(name)` | assignments + entity fields, origin service | where `name` is assigned / declared |
| `Callers` | `enclosing_callable`, `call_statements` | call sites of the enclosing function |
| `Receiver` | receiver `invoked_on` or enclosing class entity, `class_hierarchy` | class line, parents, constructor |
| `Constant(name)` | `project_ir.constants` + external constants (constants file, scraped env) | every matching key, not the first |
| `OpenFile(path)` | `ImportGraph.lookup(file, codeword)` -> `source_file` | file summary |

`Define` names come from operand-binding values (`settings.mds_url` ->
`mds_url`). `Constant(name)` is offered only when a key matches.

### OpenFile rules

- One option per resolved import of every opened file (origin first).
- Multi-hop: an opened file adds its imports.
- Visited: an opened file is never offered again (cycles end).
- Cap: 20 file options; imports mentioned in snippet, residual or bindings first.
- **Language guard:** offered only if `target.language == origin.language`.
  Origin = residual's file, fixed across hops. `fetch` re-checks.
- Target missing from IR -> dropped.
- Summary from `TypedFileRecord`: global/class assignments, one line per
  entity, callable signatures, raw REST calls. Lines naming the imported
  symbol first. Max 40 lines.

All non-file lookups stay in the origin's language and service directory.

### State

`ResidualCallState` gains `facts: Vec<Fact>` (`action`, `language`, `body`),
omitted when empty, so the triage payload is byte-identical.

### Modes

- `acquire` (A): ask -> fetch `next` -> repeat. Decide when
  `enough >= DDFA_ENOUGH`, `next == stop`, no action left, unknown `next`
  id, or `DDFA_MAX_STEPS` fetches done.
- `once` (B, baseline): fetch the first `DDFA_ONCE_FACTS` available actions
  (depth 1), ask `target` once.

Accept `target` iff choice != `none` and its probability `>= DDFA_TARGET_MIN`.

| Env | Default |
| --- | --- |
| `DDFA_MAX_STEPS` | 6 |
| `DDFA_ENOUGH` | 0.7 |
| `DDFA_TARGET_MIN` | 0.5 |
| `DDFA_ONCE_FACTS` | 12 |

Defaults are first guesses; the measurement tunes them.

### Errors

Jev HTTP error, missing origin file, no candidate service, no TypeSafe
client -> residual stays unresolved, run continues, `warn!`.

Choice limit (255 options, API): `Question::choice` never truncates; above
the limit it errors. Actions are ranked, so `available()` keeps the best 254
(+ `stop`). Services are answers, never dropped: more than 254 (+ `none`)
-> `TooManyOptions` -> residual unresolved, `warn!`. Benchmarks today: empaia
16, train-ticket 46. Tournament over service chunks when a benchmark exceeds.

### CLI

`--resolver matcher|sage|once|acquire` (default `sage` until Sage is
deleted). Takes effect with `--llm`. `once`/`acquire` require `--typesafe`.
Run metadata records the resolver.

### Logging

One `[ddfa]` row per residual in stdout and `JEV_LOG`: chosen service,
Jev calls, input tokens, action trail, target, file.

## Measurement

Arms on empaia, 5 reps each (extraction is nondeterministic):
DJ (`--resolver matcher`), DO (`once`), DA (`acquire`). Plus one DA run on
train-ticket java and poly. Scorer `jev_triage_runs/e2e-v5/score_e2e.py`.
Report median Business P/R/TP/FP/FN, Jev calls and tokens per residual.
Baseline DJ: P .88 R .88. Known ceiling: empaia FNs may live in deployment
config, so gains may show only on train-ticket.

## Out of scope

- Sage deletion: separate commit, only after the measurement and the
  user's go.
- URL output (service only).
- `OpenCallable` (function bodies) — add if misses sit in function bodies.
- RL / learned acquisition policy.
