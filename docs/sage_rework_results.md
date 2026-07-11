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
- **S1.7 — Unified triage + enforcement:** _done (2026-06-29)._ New
  `residual_edge_filter::triage(rc, project_ir, config) -> ResidualTriage`
  composes the structural gate with the edge filter into ONE decision:
  `ResidualTriage { Resolved | Empty | NonEdge | NeedsResolution }`. `Resolved`
  (gate `ResolvedURL`) and `Empty` (gate `Junk`) come straight from the gate; a
  gate `NeedsResolution` is refined via `classify_residual` into `NeedsResolution`
  (genuine cross-service residual -> forward) vs `NonEdge` (DB/dict/route noise ->
  drop). `Empty` and `NonEdge` stay DISTINCT so the two drop reasons remain
  auditable. Signals are extracted LAZILY -- only on a gate `NeedsResolution` --
  so the bulk `ResolvedURL`/`Junk` calls never pay for `signals::extract`.
  - **Enforcement.** `dispatch.rs::collect_pending_queries` now filters on
    `triage(...) == NeedsResolution` (was `is_restcall_evaluated_enough(rc) ==
    NeedsResolution`), so the ~71% non-edge population is excluded from LLM
    resolution. `evaluate_restcalls_with_llm` logs an `info!` with the non-edge
    exclusion count. Enforcement is confined to the LLM dispatch path; the non-LLM
    `evaluate` path does no resolution, so nothing there changes.
  - `classify_residual`/`ResidualEdge` unchanged (still used by `baseline.rs`'s
    measurement-only EDGE HYGIENE block, removed in S3.3).
  - **Live numbers (empaia, `--scrape`, 2026-06-29).** Confirmed end-to-end:
    gate `NeedsResolution` = 152; edge filter splits 43 cross-service / 109
    non-edge; dispatch log: `residual edge filter: excluded 109 non-edge
    residual(s) from resolution`, then `Number of REST calls to evaluate with
    LLM: 43`. So the resolver/LLM population dropped **152 -> 43** (-72%); the
    109 non-edges (DB/dict/route noise) no longer reach resolution. This is the
    first behavior change in the live path since Phase 1.

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

- **S2.2 — Service->canonical-URL rewrite:** _done (2026-06-29)._
  `query_builder.rs`. Fixed the `split('/')` bug: extraction of the path suffix
  is now a shared `path_suffix` char-scan (first `/`-leading literal, stopped at
  quote/whitespace/`+`, trailing quote trimmed) feeding `rewrite_onto_base(uri,
  base) -> base.trim_end_matches('/') + suffix`. New `rewrite_target_uri_to_service
  (uri, &ServiceDescription)` writes `<urls[0]><suffix>`; a no-url service returns
  the uri UNCHANGED so the caller treats it as an abstain. The old free-text
  `rewrite_target_uri_with_resolution` (LLM apply path) kept, now routed through
  the same fixed `rewrite_onto_base`. 6 unit tests.

- **S2.3 — Wire deterministic-only resolution:** _done (2026-06-29)._
  `dispatch.rs`. A deterministic pass runs BEFORE the LLM: for each
  `triage == NeedsResolution` residual, `signals::extract -> deterministic_match`;
  on a hit rewrite `target_uri` in place via `rewrite_target_uri_to_service`,
  applied only when `rewritten != original` (no-url match = no-op abstain). A
  resolved uri then reads as `ResolvedURL`, so `collect_pending_queries` excludes
  it from the LLM batch automatically; abstentions fall through to the LLM.

- **MILESTONE M1 / GATE B — deterministic coverage** (empaia `--scrape`, measured
  2026-06-29). Population: 577 total -> 425 ResolvedURL (73%) / 152 NeedsResolution
  (26%) / 0 Junk. Edge hygiene splits the 152 into **43 cross-service (28%)** +
  109 non-edges (71%, dropped before resolution).

  | metric | value |
  |---|---|
  | cross-service residuals | 43 |
  | matcher *hits* on cross-service | **26 / 43 (60%)** |
  | actually *resolved* (rewritten to a canonical URL) | **20 / 43 (47%)** |
  | LLM fallback population (abstentions) | **23** (= 43 − 20) |

  **The 26-vs-20 gap is real and informative.** 6 of the 26 matcher hits resolve
  to the config's `models` entry (`urls: []`, `base_dir_path: empaia/models`) —
  a shared **data-models package, not a microservice**. The generic token
  `models` (from `from empaia.models import ...` / `models.X`) token-subset
  matches it. These 6 correctly abstain (no URL -> no rewrite) and fall to the
  LLM, but they inflate the headline hit rate. The honest *resolution* rate is
  **47%**, just below the GATE B "≥60%" bar.

- **GATE B verdict** (is the LLM needed): **yes, as a fallback.** Deterministic
  resolution lands 47% (20/43) on the clean cross-service set — short of the 60%
  self-sufficiency bar — leaving 23 abstentions for the LLM closed-set classifier
  (Phase 2b). Cheap follow-up flagged: exclude no-url non-services (`models`) from
  the matcher index. It cannot lift the 20 directly (those are already resolved),
  but it stops `models` shadowing a real second-place match into a >1 ambiguous
  abstention, so it can only raise real resolution — re-measure before finalizing.

- **GATE B follow-up DONE — no-URL non-services excluded from the matcher index**
  (empaia `--scrape`, measured 2026-07-11). `matcher.rs::build_index` now filters
  `desc.urls.is_empty()`, dropping empaia's `models` (a shared data-models
  package, `urls: []`, not a microservice). Re-measured:

  | metric | before (2026-06-29) | after (2026-07-11) |
  |---|---|---|
  | det. hits on cross-service | 26/43 (60%) | **33/43 (76%)** |
  | actually resolved (rewritten) | 20/43 (47%) | **33/43 (76%)** |
  | LLM fallback population (abstentions) | 23 | **10** |
  | oracle precision / recall (spot-check) | 0.667 / 0.182 | **0.909 / 0.909** (13 edges) |

  **The unmasking was larger than predicted.** The flag expected the exclusion to
  only prevent `models` shadowing; in fact **13** residuals (33 − 20) that were
  previously abstaining — `models` collided them into a `>1` ambiguous group — now
  resolve uniquely. The hit/resolve gap also closed (33 == 33): the 6 old
  `models` hits that abstained on no-URL are gone from the index, so there is no
  longer a hit-but-can't-rewrite class. Resolution **crossed the 60% GATE B bar
  (47% → 76%)**, so the revised verdict is: deterministic tier is
  **self-sufficient for the bulk; LLM is a genuine minority fallback (10 of 43,
  23%)**, not deferrable (< 90%). The 109 non-edges are still excluded upstream by
  the edge filter; totals unchanged (577 → 425 ResolvedURL / 152 NeedsResolution).
- **Post-LLM coverage** (after S2.7): see Phase 2b below.

---

## Phase 2b — LLM closed-set classifier

- **S2.4–S2.7 — Closed-set classifier built:** _done (2026-07-11)._ The LLM stops
  GENERATING URLs and instead CHOOSES exactly one configured service (or null),
  firing only on the deterministic tier's abstentions. Implemented via a Code
  Writer subagent (report: `agents/reports/sage-phase2b-writer.md`); orchestrator
  reviewed all core diffs + ran the end-to-end validation below.
  - **`sage` crate reshaped.** `QueryKind` collapsed to one variant
    `ClassifyTargetService { candidates: Vec<CandidateService{name,url}> }`;
    `SageQuery { kind, context: ClassifyContext }` (retires `FactBundle` +
    `variables_map`); `SageResponse { service: Option<String>, evidence,
    reasoning }` (confidence DROPPED); `validate()` = membership (`service ∈
    candidates ∪ {null}`) + evidence grounding (a cited token is a
    case-insensitive substring of the call-site context); null service = valid
    abstain. `prompt.rs` rewritten to §6.1/§6.2 (closed-set contract +
    precedence-ordered CALL SITE / CANDIDATE SERVICES); `build_variables_message`
    /prose hint deleted.
  - **Structured-output enforcement.** `client.rs` sends async-openai 0.28
    `ResponseFormat::JsonSchema` with the §6.3 schema constraining `service` to
    the candidate-name enum + null (`strict: false` — Ollama rejects
    strict+nullable-enum; `validate` enforces membership regardless).
  - **Candidates exclude origin AND no-URL services** (`query_builder.rs`),
    mirroring the matcher; `dispatch::apply_query_outcomes` maps the chosen
    service name → `ServiceDescription` → `rewrite_target_uri_to_service`
    (abstain/error/not-in-config all leave the residual untouched).
  - **Cleanup pulled forward** (the migration orphaned them; project bans
    `#[allow(unused)]`): deleted `sage/resolver/{facts,code}.rs`,
    `llm_enhance/ranking.rs` (`rank_and_cap` + `build_snippet`), `variables.rs`'s
    `build_variable_map`. `baseline/oracle/scorer` kept (removed in S3.3).
  - **Green:** `cargo test -p sage` 13 pass (+2 ignored live-Ollama), `-p
    extractor-runtime` 54 lib + 48 integration pass; clippy no NEW warnings (the
    pre-existing `dispatch.rs &mut Vec` was fixed to `&mut [_]` in passing; only
    the 2 extractor `as_bytes` warnings remain).

- **End-to-end validation** (empaia `--scrape --llm`, live Ollama
  `qwen2.5-coder:7b` — same model as the 0%-precision June run; measured
  2026-07-11, `SAGE_TRACE`):

  | metric | value |
  |---|---|
  | LLM queries dispatched (= deterministic abstentions) | 10 |
  | **invalid JSON raw responses** | **0 / 10** |
  | rejected (non-candidate / ungrounded) | 0 / 10 |
  | chose a service | 1 |
  | abstained (`null`) | 9 |

  **The core mechanism works.** **0/10 invalid JSON vs 53% in June** — the
  `response_format` json-schema enforcement eliminates the invalid-JSON failure
  mode at the source. No hallucinated non-candidates (0 rejected). The one
  resolution — expression `url` → `examination-service`, evidence
  `["examination","models.v1.examinations"]` — is grounded on the file's import
  and plausible (unscoreable by the thin oracle: bare `url` has no `*_url`
  constant). The 9 abstentions are APPROPRIATE, not misses: the LLM tail is the
  thin/infra/malformed residual set the deterministic matcher already couldn't
  close — `{settings.vault_url}/...` (HashiCorp Vault, infra), `registry_image_url`
  (Docker registry), `url"]"/alive` and `None/private/...` (extraction artifacts).
  Conservative abstention on non-service targets is the precision-first behavior
  the closed-set design intends. Empaia's low LLM yield is expected (§4 Phase 0:
  strong-signal cases resolve deterministically; the thin tail has no value
  path); **train-ticket is where the LLM must carry load** (Phase 3 / GATE C).

---

## Phase 3 — Final evaluation

- **S3.1 — Scorer wired:** _done (2026-07-11)._ Added a `SAGE_SCORE=<constants-file>`
  gate in `dispatch::evaluate_restcalls_with_llm` (snapshot each residual's operand
  identifiers BEFORE rewrite; after deterministic + LLM resolution derive the final
  chosen service from `service_for_url(target_uri)`; `score()` vs the auto-oracle).
  Two `pub(super)` helpers in `oracle.rs` (`from_constants_file`, `service_for_url`).
  Green: `-p extractor-runtime` build + clippy clean, 45 llm_enhance tests pass (2 new).
  - **Redundancy discovered:** `baseline.rs::log_deterministic_coverage` ALREADY
    scored via the oracle, gated on `SAGE_ORACLE_CONSTANTS`+`SAGE_ORACLE_CONFIG` —
    but over the **deterministic-only** output (pre-LLM). The new `SAGE_SCORE` path
    scores the **LLM-final** output and survives `baseline.rs`'s S3.3 removal. The
    two env-var names were reconciled in S3.3 (see below): `SAGE_ORACLE_*` went away
    with `baseline.rs`, leaving `SAGE_SCORE` as the single scorer path.

- **S3.2 — End-to-end runs** (live Ollama `qwen2.5-coder:7b`, `--scrape --llm`,
  relative `-p ../<corpus>` + `local-*-config.json`; measured 2026-07-11):

  **Empaia** (577 restcalls):

  | stage | count |
  |---|---|
  | ResolvedURL | 449 (77%) |
  | NeedsResolution (residual) | 128 (22%); strong 127 / thin 1 |
  | edge hygiene | 19 cross-service / 109 non-edge |
  | deterministic resolved | **10 / 19 (52%)** |
  | dispatched to LLM | 9 |
  | LLM: chose / abstained / error / **invalid JSON** | 2 / 6 / 1 / **0** |
  | **oracle score** | **0 scoreable** (13-edge oracle, 0 dropped) |

  **Train-ticket** (641 Java/Py files):

  | stage | count |
  |---|---|
  | NeedsResolution (residual) | 24; strong 24 / thin 0 |
  | edge hygiene | 22 cross-service / 2 non-edge |
  | deterministic resolved | **0 / 22 (0%)** |
  | dispatched to LLM | 22 |
  | LLM: chose / abstained / error / **invalid JSON** | 18 / 1 / 3 / **0** |
  | spot-check on the 18 choices | **~16 grounded-correct, 2 wrong** |
  | oracle score | N/A (no train-ticket constants file) |

  Reconciliation vs Phase 2's 152/43/33 empaia figures: this run's `--scrape` over
  `../empaia` statically resolved ~24 more URLs (449 vs 425 ResolvedURL), shrinking
  the residual set (128 vs 152) and the cross-service tail (19 vs 43), leaving a
  harder remainder — hence 52% deterministic, not 76%. Not a regression; the static
  layer got MORE, the deterministic tier got the leftover.

- **GATE C verdict (threshold: train-ticket precision < 0.7 → revisit §6.4):**
  **soft-PASS, rigorous verdict DEFERRED.** The auto-oracle is **0 scoreable on both
  corpora**, so it produces no scored precision (see finding 1). The train-ticket
  spot-check on the 18 LLM choices is ~16/18 ≈ **0.89** grounded-correct — above 0.7
  — but on a WEAK population (finding 2) and with 2 real candidate-confusion errors
  (finding 3). Per the 2026-07-11 decision (auto-oracle both, extend later), the
  rigorous verdict is deferred to a hand-labeled pass; the soft result does not
  justify reopening §6.4 narrowing now, but the price-service confusion is the exact
  failure mode narrowing would address if a labeled run confirms it.

- **Findings (threats to validity — feed the eventual hand-labeled pass):**
  1. **Auto-oracle structurally misses the current residual tail.** The oracle keys
     on `*_url` constant NAMES (`mds_url`, `self._cds_url`); it looks them up against
     each residual's `operand_identifiers`. But post-gate residual operands are path
     params (`annotation_id`, `class_id`, bare `url`) — the discriminating token has
     moved to `client_class`/`imports`, which the oracle does not index. So 0
     scoreable even for empaia. Real precision needs hand-labeling (or an oracle that
     joins on imports/class, not just operands).
  2. **Train-ticket residual population is weak.** ≥6/22 are in `src/test/` files,
     and most carry the full `http://ts-<name>-service:port/...` literal in the
     expression — so the LLM largely READS the service name from the URL rather than
     reasoning. Several arguably should be `ResolvedURL` (host already names the
     service): a gate / symbolic-eval fold-gap worth a separate look.
  3. **Validation gap: grounded ≠ correct.** For `http://ts-price-service:16579/...`
     the LLM chose `ts-consign-price-service` (both are candidates) with evidence
     citing `ts-price-service` — grounding passed because the cited token is in the
     context, but it justifies a DIFFERENT service than the one chosen. `validate()`
     checks token-in-context, not token-agrees-with-choice. 2/18 such errors.

- **Extraction-artifact fix (root-cause of findings 2 & 3):** _done (2026-07-11)._
  Traced the train-ticket residual population to a single defect: when
  `symbolic_evaluation_with_env` fails for a REST call (e.g. a Java **test method**
  whose mangled name is absent from the callable map — 20/20 train-ticket eval
  failures), `pass3/restcalls.rs`'s `Err` branch **preserved the raw template
  verbatim**, so a fully-literal `"http://ts-x-service:port/..."` kept its
  surrounding quotes, failed the gate's `starts_with("http")`, and was misfiled
  `NeedsResolution` → cross-service → LLM. Fix: the `Err` branch now runs
  `generate_uris` against an empty `AnalysisResult` (pure literals need no method
  env), stripping inline-literal quotes and concatenating literal parts; genuinely
  env-dependent variables still fall through and stay residual.
  - Changes: `Expr` derives `Default` (`Empty` is `#[default]`) in `models`;
    `AnalysisResult` derives `Default` in `statix`; the `Err` branch in
    `extractor-runtime/pass3/restcalls.rs`. 3 new `uri_generator` unit tests
    (pure-literal + concatenated-literal resolve; unbound variable stays
    unresolved). Green: build + clippy clean; `extractor-runtime` 56+48, `statix`
    13+71, `java-extractor` 33 tests pass.
  - **Effect on train-ticket** (re-run 2026-07-11):

    | metric | before fix | after fix |
    |---|---|---|
    | ResolvedURL | 213 (89%) | **233 (98%)** |
    | NeedsResolution | 24 | **4** |
    | cross-service residuals | 22 | **2** |
    | dispatched to LLM | **22** | **2** |

    The 20 quote-wrapped literals now resolve **statically to the correct host**,
    so the entire `ts-price-service`→`ts-consign-price-service` LLM error class
    (finding 3) is eliminated at the source, and the LLM tail is the 2 genuinely
    variable-based calls (`requestUrl` etc.) that *should* reach it — the first of
    which the LLM abstained on, appropriately.
  - **Empaia unaffected:** 13 `Err`-branch cases, **0 quoted-http** — the fix folds
    none of them (they are variables/path params, not quoted literals); bucket
    classification is unchanged, so no re-run was needed.
  - **Bigger lesson for the paper:** train-ticket's raw "22 cross-service residuals"
    were ~91% extraction artifacts. Evaluation populations must be built *after*
    this fix; the true unresolved-call count is an order of magnitude smaller than
    the raw gate output suggested. (Separate, still-open scoping question: whether
    `src/test/` calls should contribute SDG edges at all — finding 2's other half.)

- **S3.3 — Cleanup:** _done (2026-07-11)._ The plan's literal dead-machinery list
  (`ranking.rs::rank_and_cap`, dead `QueryKind`s, `FactBundle` snippet shape,
  `variables_budget`, prose hint, `build_snippet`) was **already removed** across
  Phases 1–2; a repo-wide grep confirms none survive. `QueryKind` is now an
  intentional single-variant enum (the documented closed-set collapse), not dead.
  What remained:
  - **Removed `baseline.rs`** (293 lines, the TEMPORARY S0.3 instrumentation whose
    own header slated it for S3.3 removal): the BASELINE-BUCKETS / STRONG-VS-THIN /
    DETERMINISTIC-COVERAGE / EDGE-HYGIENE / ORACLE-SCORE log blocks. Their numbers
    are already captured in this document; the permanent lightweight diagnostics
    (`residual edge filter: excluded N`, `deterministic resolver: resolved X of Y`,
    `scorer: precision … recall …`) live in `dispatch.rs` and stay.
  - **Env-var unification:** `SAGE_ORACLE_CONSTANTS`/`SAGE_ORACLE_CONFIG` were used
    only by `baseline.rs` and disappeared with it. `SAGE_SCORE=<constants-file>` (the
    LLM-final scorer in `dispatch.rs`) is now the **single** scoring env var.
  - **Orphan swept:** `ServiceOracle::load` (two-file disk loader) had `baseline.rs`
    as its only non-test caller; removed it and re-pointed its test
    (`empaia_known_edges_resolve`) at `from_constants_file` (parses the config in the
    test, then joins). `from_constants_file` + `service_for_url` remain the load path.
  - Stale doc comments referencing `baseline.rs` / `load` were fixed
    (`residual_edge_filter.rs`, `oracle.rs`). Green: `-p extractor-runtime` build +
    clippy clean, 56 unit + 48 integration tests pass.

- **S3.4 — Final doc polish:** pending.

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
- **2026-06-29 — GATE B = LLM needed as fallback.** With the population clean
  (43 cross-service), deterministic resolution lands 20/43 (47%), below the 60%
  self-sufficiency bar -> keep the LLM closed-set fallback (Phase 2b) for the 23
  abstentions. Finding: the matcher *hits* 26/43 but resolves only 20 — 6 hits go
  to the config's `models` entry, a data-models package (`urls: []`,
  `base_dir_path: empaia/models`), not a microservice; they abstain on no-url.
  Follow-up flagged (not yet done): exclude no-url non-services from the matcher
  index, then re-measure (can unmask real second-place matches, only raises
  resolution).
- **2026-07-11 — Keep `src/test/` calls in the analysis (do NOT exclude).** The
  goal includes analyzing tests for Regression Test Selection, so test-file REST
  calls are wanted, not noise. The extraction-artifact fix now resolves their
  literal URLs statically to the correct target service — exactly the test→service
  mapping RTS needs. Open (deferred) design nuance: tag test-originated SDG edges
  (origin `src/test/` vs `src/main/`) so they don't pollute the production
  architecture view — same graph, extra attribute. Not implemented.
- **2026-07-11 — Defer the `validate()` tightening (finding 3 / grounded ≠
  correct).** The closed-set check grounds evidence in the call-site context but
  does not require the evidence to point at the *chosen* service, so a grounded-
  but-wrong pick (chose `ts-consign-price-service`, cited `ts-price-service`)
  passes. A correct rule must mirror the matcher's acronym + token-subset logic
  (a plain substring rule would over-reject valid non-lexical groundings like
  `mds_url` -> `medical-data-service`). Deferred: the extraction-artifact fix
  removed every observed instance, making this a low-stakes hardening item.
