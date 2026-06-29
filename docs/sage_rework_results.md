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
  - **Correction:** `project_ir.callable_map` (the *global* map from
    `pass2::callables::build_project_global_callables`) is keyed by **mangled
    name only** — it never inserts the hash, so a hash lookup against it always
    misses. (The separate *file-local* map from `build_file_local_callables` is
    keyed by **both** mangled name and `metadata.hash`; that is what
    `restcalls.rs` uses.) The plan/§5 assumption conflated the two. Class is
    recovered by scanning the owning file's `callables` for
    `metadata.hash == rc.function_hash`. See memory `callable_map_keying.md`.
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
- **S0.2 — Ground truth (auto-derived oracle) + scorer:** _done (2026-06-22)._
  New `llm_enhance/oracle.rs` + `llm_enhance/scorer.rs`.
  - **Oracle join (NOT circular, NOT hand-supplied).** Derived by joining the
    curated constants file (identifier -> URL) with the config file
    (URL -> service) on **host**, never the full URL. `host_of` strips scheme,
    `:port`, and any path. Config builds host -> {service names}; each constant's
    URL host is looked up. Exactly-one-service host -> emit edge
    `normalize(identifier) -> service`; zero/ambiguous host -> drop (oracle is
    deliberately partial). Joining on host (not URL) is what tolerates the
    expected port mismatch (constants `mds_url -> ...:5000`, config `...:8000`).
  - **normalize(identifier):** keep last dotted segment (drops `settings.` /
    `self.`), strip leading `_`, lowercase. `settings.mps_url` -> `mps_url`,
    `self._mds_url` -> `mds_url`, `_mds_url` -> `mds_url`. Aligns with how a
    residual's `operand_identifiers` look so the scorer can join them.
  - **Empaia edges derived:** 25 constants rows -> **13 unique oracle edges**,
    **0 dropped**. The 25->13 collapse is normalization folding prefixed/
    underscored variants onto one key (e.g. `mps_url`, `settings.mps_url`,
    `self._mps_url` -> `mps_url`). All 25 hosts resolve to exactly one config
    service (incl. `marketplace-service-mock` host -> `marketplace-service`), so
    nothing dropped on empaia. Spot-checked edges: `mds_url ->
    medical-data-service`, `cds_url -> clinical-data-service`, `es_url ->
    examination-service`, `as_url -> annotation-service`.
  - **expected_service(idents):** normalize each, look up; agree on one ->
    that service; none match -> None; conflict -> None (unscoreable).
  - **Scorer semantics (pure; reused verbatim in Phase 3).** A residual is
    *scoreable* iff `expected_service` is `Some` (only these count toward
    recall's denominator). `produced` = scoreable residuals where the matcher
    chose a service; `correct` = chosen == expected; `precision = correct /
    produced`; `recall = correct / scoreable`; both divisions guarded against
    zero (no NaN).
  - Reachability: `baseline.rs` now logs deterministic coverage and (when
    `SAGE_ORACLE_CONSTANTS` + `SAGE_ORACLE_CONFIG` env vars are set) the oracle
    precision/recall. Observation-only; removed in S3.3 with the rest of
    `baseline.rs`. Green: 50 lib unit tests pass (incl. 23 new across matcher/
    oracle/scorer/tokens), clippy clean (only pre-existing `dispatch.rs:33
    &mut Vec`).
- **S0.3 — Baseline buckets** (old lexical gate; measured 2026-06-22 via
  temporary `llm_enhance/baseline.rs`, observation-only). Run before any network
  dispatch, so independent of Ollama/sage availability.

  | Bucket | empaia (577 total) | train-ticket (237 total) |
  |---|---|---|
  | `Enough` (http) | 449 (77%) | 213 (89%) |
  | `NeedsLLM` (passes url/uri test) | 18 (3%) | 2 (0%) |
  | `Junk:empty` | 0 (0%) | 0 (0%) |
  | **`Junk:non-empty`** (silent recall hole) | **110 (19%)** | **22 (9%)** |

- **S0.4 — Strong-vs-thin split** (residual = `NeedsLLM` + `Junk:non-empty`):

  | | empaia | train-ticket |
  |---|---|---|
  | residual total | 128 | 24 |
  | strong (class **or** import-token overlap) | 127 (99%) | 24 (100%) |
  | thin | 1 (0%) | 0 (0%) |
  | empty `function_hash` | 0 (0%) | 0 (0%) |

  Note: empaia's high "strong" rate is partly driven by the *loose* import-token
  overlap heuristic (route files import many same-service-named modules), not
  only `client_class` (which S0.1 measured at 48/128). The matcher's real
  precision on these is a Phase 2 / GATE B question, not settled here.
- **S0.5 — Gate thresholds committed:** see section above.
- **GATE A verdict: CONFIRMED — the lexical gate is the leak.** On both corpora
  `Junk:non-empty` dominates the residual set and dwarfs `NeedsLLM`: empaia
  forwards 18 to the resolver while silently junking 110 (~6x); train-ticket
  forwards 2 vs 22 junked (~11x). ~99-100% of those junked residuals carry a
  structural signal, so they are recoverable, not noise. **Phase 1 gate rework
  is justified and is the highest-leverage next step.** `function_hash` is never
  empty on residuals (0/128, 0/24) -> the S0.1 empty-hash fallback gap is a
  non-issue on these corpora; deprioritize.

---

## Phase 1 — Structural gate

- **S1.1-S1.3 — Gate reworked** (2026-06-22). Lexical `url`/`uri` substring test
  replaced by a structural rule in `restcalls.rs`. `EvalState` is now
  `ResolvedURL | NeedsResolution | Junk`:
  - empty `target_uri` -> `Junk`
  - starts with `http` -> `ResolvedURL`
  - any other non-empty residual -> `NeedsResolution`

  Rationale: a `RestCall` already IS an HTTP call (it carries `http_method`), so
  any non-empty residual eval did not turn into a URL is a real residual the
  Phase 2 matcher should try, regardless of naming. `dispatch.rs` filter now
  selects `NeedsResolution`. 6 unit tests added.
- **S1.4 — Post-gate buckets** (new structural gate; measured 2026-06-22 via
  `llm_enhance/baseline.rs`, observation-only, before any network dispatch).

  | Bucket | empaia (577 total) | train-ticket (237 total) |
  |---|---|---|
  | `ResolvedURL` (http) | 449 (77%) | 213 (89%) |
  | `NeedsResolution` (non-empty residual) | 128 (22%) | 24 (10%) |
  | `Junk:empty` | 0 (0%) | 0 (0%) |
  | **`Junk:non-empty`** | **0 (0%)** | **0 (0%)** |

  Totals match the S0.3 baseline (577 / 237) -> same invocation, same population.

- **Recovery — `NeedsResolution` vs old `NeedsLLM`:**

  | Corpus | old `NeedsLLM` | new `NeedsResolution` | recovery |
  |---|---|---|---|
  | empaia | 18 | 128 | ~7.1x (= 18 + 110 old `Junk:non-empty`) |
  | train-ticket | 2 | 24 | 12x (= 2 + 22 old `Junk:non-empty`) |

  The new `NeedsResolution` count is exactly the old `NeedsLLM` plus the old
  `Junk:non-empty` (the silent recall hole). No residual is lost or added.

- **Refreshed strong-vs-thin split** (residual population = `NeedsResolution`):

  | | empaia | train-ticket |
  |---|---|---|
  | residual total | 128 | 24 |
  | strong (class **or** import-token overlap) | 127 (99%) | 24 (100%) |
  | thin | 1 (0%) | 0 (0%) |
  | empty `function_hash` | 0 (0%) | 0 (0%) |

  Unchanged from S0.4 -> the population is identical (S0.4 already measured over
  `NeedsLLM` + `Junk:non-empty`, which is exactly the new `NeedsResolution` set).

- **`Junk:non-empty` is now structurally 0** on both corpora. Under the new
  gate `Junk` is only returned for an empty `target_uri`, so a non-empty residual
  can never be junked -> the silent recall hole is closed by construction, and
  the 0 is the proof. (`baseline.rs` keeps the four-bucket breakdown to
  demonstrate this; it is removed in S3.3 cleanup.)

---

## Phase 1.5 — Residual edge hygiene

- **S1.5 — Residual edge classifier:** _done (2026-06-29)._ New
  `llm_enhance/residual_edge_filter.rs`: `ResidualEdge { CrossService, NonEdge }`
  via `classify_residual(rc, &signals)`. Kept DISTINCT from the gate's `Junk`
  (empty) state so dropped calls stay auditable. Splits the `NeedsResolution`
  population by target URL-nature into genuine cross-service HTTP edges vs
  non-edges (intra-service DB clients `ClassClient.get(class_id)`,
  route-internal reads `clients.annotation.get(annot_id)`, dict `.get(bare_var)`)
  that the lexical REST-call identifier swept in.
  - **Rule — `CrossService` iff ANY of:** (1) `target_uri` contains `http`; (2)
    `target_uri` contains `/` (a path); (3) any operand identifier hints a URL
    (uppercased contains `URL`/`URI`/`HOST`/`ENDPOINT`/`BASE`/`PORT` -- mirrors
    `ranking.rs::name_hints_url`, replicated inline since `ranking.rs` is deleted
    in Phase 3). Otherwise `NonEdge`.
  - **Rule 4 dropped (S1.6, 2026-06-29).** A 4th rule (operand token matches a
    candidate service NAME) was implemented, then removed when the first live run
    showed it INVERTED the split (127 cross / 25 non-edge). Domain nouns ARE the
    service names: a local `annotation_id` shares `annotation` with
    `annotation-service`; `class_id` shares `id` with `id-mapper-service`. No
    lexical way to separate a local id-param read from a call to the same-named
    service, so the rule false-flagged path-params as edges. The agent's unit
    test used placeholder service names without those tokens, so it passed in
    isolation -- only the live empaia config exposed it. Target URL-nature
    (rules 1-3) is the discriminator that survives.
  - **Known coarseness (acceptable, measurement-first):** the rule-3 substring
    list over-matches -- `BASE` hits `database`, `PORT` hits `report`. None of
    empaia's observed non-edge operands (`class_id`, `annotation_id`, `item_id`,
    `collection_id`) trip these, so it is not corrected here.
  - **MEASUREMENT-ONLY.** Wired into `baseline.rs::log_deterministic_coverage`
    as an observation-only `EDGE HYGIENE` block (cross-service / non-edge counts
    + deterministic coverage restricted to the cross-service subset). Nothing the
    matcher or dispatch sees changes. Enforcement (drop `NonEdge` before
    resolution) is **S1.7**. Removed in S3.3 with the rest of `baseline.rs`.
- **S1.6 — Edge-hygiene split** (empaia, `--scrape`, measured 2026-06-29 via
  `baseline.rs`, observation-only; rules 1-3 after rule-4 removal):

  | | empaia (`--scrape`) |
  |---|---|
  | `NeedsResolution` (residual total) | 152 |
  | **cross-service** (genuine edges) | **43 (28%)** |
  | **non-edge** (DB/dict/route noise) | **109 (71%)** |
  | deterministic hits on cross-service | **26 (60% of 43)** |

  **Reading.** The polluted `NeedsResolution` set is **71% non-edges** -- the
  lexical REST-call identifier swept in intra-service DB/dict/route reads. Only
  **43** residuals are genuine cross-service calls. The matcher fired 77 over the
  whole set but only **26** land on the clean cross-service subset; the other
  ~51 fires are FALSE edges that S1.7 enforcement removes. Deterministic coverage
  on the real population is **60%** (26/43) -- right at the GATE B "≥60% ->
  LLM-fallback-only" line, and now an INTERPRETABLE number (vs the meaningless
  77/152 over the polluted set). Caveat: 60% is *coverage*, not precision;
  precision is still only spot-checkable on the 11-sample oracle (2/3). The 43
  cross-service set is what Phase 2 should actually resolve.

---

## Phase 2 — Service matcher

- **S2.1 — Deterministic identifier->service matcher:** _done (2026-06-22)._
  New `llm_enhance/matcher.rs`; shared splitters extracted to
  `llm_enhance/tokens.rs` (`split_camel`/`split_snake` moved out of `ranking.rs`,
  which now calls them -> ranking behavior byte-identical, its tests still pass;
  survives the Phase 3 `ranking.rs` deletion).
  - **Index** over `config.service_descriptions`: each service reduced to a
    *token set* (split name on non-alnum + camel/snake, lowercase, strip generics
    {service,client,url,uri,api,http}) and an *acronym* (first char of each FULL-
    name token before stripping). `medical-data-service` -> tokens {medical,data},
    acronym `mds`; `event-service` and `examination-service` both -> acronym `es`.
  - **Signal normalization.** `client_class` (e.g. `MedicalDataServiceClient`):
    camel-split, strip generics -> {medical,data}; matches a service iff the
    service's stripped tokens are a non-empty subset of the class tokens.
    `imports`: same tokenization, token-subset. `operand_identifiers` (e.g.
    `self._mds_url`, `cds_url`): strip leading `self`/`this`, leading `_`, and a
    trailing `_url`/`_uri` -> key (`mds`, `cds`); match by **acronym equality**
    OR token-subset (so a full word like `annotation` also matches).
  - **Precedence + abstention (high precision, partial recall).** Groups tried
    strongest-first [client_class, imports, operand_identifiers]; in each, hits
    EXCLUDE `origin_service` (no self-loops); 1 hit -> accept; 0 -> fall through;
    >1 -> abstain (None, defer to LLM). Returns `Option<ServiceDescription>`
    (clones the service so S2.2 has its `urls`).
  - **§5 worked examples all pass (unit tests):** (1) class
    `MedicalDataServiceClient` -> medical-data-service (accept); (2) `cds_url`
    acronym -> clinical-data-service (accept); (3) `es_url` with both event- and
    examination-service -> ambiguous -> None; (4) `es_url` + class
    `ExaminationServiceClient` -> class fires first -> examination-service
    (accept); (5) `base_url`, no class/import -> None; (6) only-hit-is-origin ->
    excluded -> None. Plus import-token-subset and full-word-operand accept.
  - Reachable via `baseline.rs::log_deterministic_coverage` (observation-only).

- **M1 PREVIEW — deterministic coverage + oracle score** (empaia only, measured
  2026-06-29 via `baseline.rs::log_deterministic_coverage`, observation-only).
  Two ingestion modes, because mode decides the population:

  | | `-f` curated constants | `--scrape` (real ingestion) |
  |---|---|---|
  | ResolvedURL | 449 (77%) | 425 (73%) |
  | **NeedsResolution** | 128 (22%) | **152 (26%)** |
  | Deterministic coverage | 61/128 (47%) | 77/152 (50%) |
  | Oracle scoreable | **0** | **11** |
  | produced / correct | 0 / 0 | 3 / 2 |
  | **precision / recall** | — | **0.667 / 0.182** |

  The `--scrape` row is the meaningful one; `-f` pre-resolves the cross-service
  calls (they become `ResolvedURL`), leaving 0 scoreable.

  **Findings (the preview's real payload — see Decision log 2026-06-29):**
  1. **Oracle too thin to gate.** Scores only 11/152 (~7%); curated-vs-scrape
     delta caps it at ~24. P/R on 11 samples are directional, not a verdict.
  2. **Matcher fires mostly on noise.** Of 77 fires, only **3** land on
     scoreable residuals; the rest hit non-cross-service calls.
  3. **`NeedsResolution` is polluted with non-edges.** Sample residuals are bare
     path-params (`annotation_id`, `class_id`, `item_id`, `collection_id`) from
     FastAPI route bodies and intra-service DB clients (`ClassClient` +
     asyncpg), not outbound HTTP. Root cause: REST-call identification is purely
     lexical (`python-extractor`: any `*.get/post(arg)`). `JobClient ->
     job-service` was confirmed a **false edge** (`JobClient` is an asyncpg DB
     client). -> motivates **Phase 1.5 (residual edge hygiene)**.

- **GATE B verdict** (is the LLM needed): _blocked — not measurable until the
  residual population is cleaned (Phase 1.5). Coverage on a polluted set is
  uninterpretable._
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
- **2026-06-22 — NEXT ACTION (reorder).** Do **Phase 1 (gate rework, S1.1-S1.4)
  next**, NOT S0.2, despite the plan's literal order. Reason: Phase 1 changes
  the residual population (empaia 18 -> 128 forwarded), so building S0.2's
  ground-truth/scorer against the old-gate population would have to be redone.
  **S0.2 (scorer + auto-derived oracle) is deferred to immediately before M1**
  (Phase 2 deterministic coverage), measured against the real post-gate
  population. Phase 1 needs no oracle. Resume a cold session by spawning a Code
  Writer for S1.1-S1.4 (see plan §9 Phase 1).
- **2026-06-29 — Oracle demoted to spot-check.** empaia has no published
  dependency graph and hand-labeling is ruled out, so the curated constants file
  is the only automatic labeler — but it scores only ~7% of residuals (11/152;
  ~24 ceiling). It is reliable where present (URL values are deployment fact,
  not opinion) and non-circular, but too partial to base GATE B/C on. Keep
  `oracle.rs`/`scorer.rs` as a documented spot-check; do not widen them.
- **2026-06-29 — NEXT ACTION (insert Phase 1.5 before Phase 2 enforcement).**
  The M1 preview's dominant finding is population pollution, not precision: the
  `NeedsResolution` set is full of non-edges (intra-service DB clients, route
  bodies, dict `.get`) because REST-call identification is purely lexical. The
  matcher fires 74/77 on this noise. Fix the **population** before the LLM leaf
  or any GATE B verdict. Added **Phase 1.5 (residual edge hygiene)**: classify
  residuals by target URL-nature into genuine cross-service vs `NonEdge`,
  measurement-first (S1.5/S1.6), then enforce (S1.7). This is ground-truth-free
  — the right lever given the thin oracle. Resume by spawning a Code Writer for
  S1.5 (see plan §4 Phase 1.5 / §9 Phase 1.5).
