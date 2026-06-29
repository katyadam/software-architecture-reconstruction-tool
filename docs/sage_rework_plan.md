# Sage Resolution Rework — Implementation Plan

The consolidated plan for reworking how unresolved REST-call targets are
resolved into service edges. Distilled from the diagnosis in
[`sage_resolution_redesign.md`](sage_resolution_redesign.md), the
[`sage_ingestion_audit.md`](sage_ingestion_audit.md), and the empirical
[`sage_validation_findings.md`](sage_validation_findings.md).

- **Date:** 2026-06-22
- **Branch:** `sage-rework` (off `llm-sage`)
- **Thesis:** ingestion is *not* the blocker — config values are already scraped.
  The blockers are *consumption* (available values not applied), an
  *over-pruning lexical gate*, and the *wrong question* asked of the LLM. The
  rework widens the residual population, resolves the service deterministically
  where possible, and asks the LLM a closed-set classification question only for
  the ambiguous tail.

---

## 1. Architectural boundary (fixed)

The **extractor** is responsible for producing fully-resolved, URL-shaped
`target_uri`s. The **synthesizer is not touched** — it keeps consuming
URL-shaped targets and matching them to endpoints/services exactly as it does
today (`sdg/builder.rs`: `exact_match` then Levenshtein against
`service.url + endpoint.uri`).

Consequence: whatever the resolution path determines — even when it determines a
*service* rather than a literal URL — it must be written back into `target_uri`
as a URL (the service's canonical URL from config, plus any resolved path
suffix) before handoff. No new fields on `RestCall`, no synthesizer changes.

---

## 2. What already works (do not rebuild)

- **Deterministic URL resolution exists.** `pass3/restcalls.rs::evaluate_single_restcall`
  builds `eval_env` (project constants + `external_constants` + per-file attrs +
  module consts + captured scopes), runs `symbolic_evaluation_with_env`, and
  `generate_uris` substitutes resolved values into `target_uri`. When the env
  holds the value, `self._mds_url + url` already becomes a full URL.
- **Config ingestion exists.** `env-scraper` parses `.env*` and
  `docker-compose*.{yml,yaml}`; values reach the extractor as
  `external_constants` via `--scrape` or `-f constants.json`.
- **URL -> service edge exists.** The synthesizer matches a URL `target_uri`
  against `service.urls` + endpoint URIs.

The rework therefore touches **only the residual path** — restcalls where
symbolic evaluation produced no `http` URL — plus the gate that selects them.

---

## 3. Target architecture

```
[extractor]
symbolic eval + generate_uris   (EXISTING deterministic URL resolution)
  └─ structural gate            [Phase 1]  -> ResolvedURL | NeedsResolution | Junk
        ├─ ResolvedURL ──────────────────────────────────────────────┐
        └─ NeedsResolution                                            │
              └─ service matcher [Phase 2]                            │
                    ├─ deterministic identifier→service  (no LLM)     │
                    └─ LLM closed-set classify           (fallback)   │
                          └─ service -> config canonical URL ─────────┤
                                                                      ▼
                                           target_uri = URL (+ suffix)
[synthesizer]  UNCHANGED  ──► URL→endpoint→service ──► SDG / IMCG / CM
```

The two hard pieces are the **gate** (Phase 1) and the **LLM leaf** (Phase 2b).
Everything else makes the leaf reachable, trustworthy, and consumable.

---

## 4. Phases

> **Execution discipline (per-phase rule).** This plan is executed phase by
> phase across context compactions, so the **repo is the only durable memory**.
> Every phase must therefore end:
> 1. **Green** — compiles, `cargo clippy` clean, tests passing.
> 2. **Committed** — a self-contained commit on `sage-rework`.
> 3. **Recorded** — every measurement / gate result appended to
>    `docs/sage_rework_results.md` (numbers, the gate verdict, and the decision
>    taken). Never leave a gate result living only in the session.
>
> A later cold session resumes from git + `sage_rework_results.md` alone. If a
> result is not written down, it did not happen.

### Phase 0 — Foundation + baseline (precursor)

**Decision: no new static analysis.** The residuals that symbolic evaluation
cannot close — notably `self.`-attribute targets like `self._mds_url + url` —
are **not** resolved to a literal value. They flow to the Phase 2 service
matcher and are resolved to a **service** (deterministic class/acronym match,
LLM closed-set fallback). We do **not** build a constructor-injection pass, and
we do **not** ask the LLM to generate the URL value.

**Why this is sound** (from the env-build investigation, `env.rs` +
`constants.rs` + `pass_attr.rs`):

- `self.`-attribute values resolve today **only** when `external_constants`
  literally contains the key (e.g. the curated `empaia-constants.json`); the
  real `--scrape` path never produces it, and **pass_attr structurally cannot**
  (it handles only global `var = ClassName()` bindings). Closing that statically
  means a cross-file constructor-injection pass — which we are choosing not to
  build.
- It would also be **largely redundant**: the `self._mds_url` residual lives
  *inside* the client class `MedicalDataServiceClient`, so the strongest
  classification signal (the class name) is already in scope. These are the
  *easy* cases for Phase 2a, not the hard ones.

**Consequence to accept:** with no value path, a residual whose name/class
signal is weak **and** has no recoverable value has no fallback — the LLM
classifies from thin signal or the edge is missed. Small on empaia (strong
names); the risk concentrates on train-ticket (40 near-identical `ts-*` names).
That is what Phase 3's train-ticket measurement is for; only if it fails do we
reconsider any static resolution.

Phase 0 is **not** "log some counts." It is the foundation the gates depend on:
without ground truth and a scorer, GATE B at M1 has nothing to measure against,
and without persisted results nothing survives compaction. It also builds the
signal extractor early — both because S0.3 needs it and because it is the
matcher's foundation, best de-risked before Phase 2 commits to it.

**Do in this phase:**

1. **Signal extractor** (this is S2.0, pulled forward). From `RestCall` +
   `ProjectIR` + `ConfigurationData`: origin service, client class, file
   imports, operand identifiers, candidate services. Validate it produces real
   signals on actual empaia residuals.
2. **Ground truth** — the correct source→target service edges per corpus
   (empaia first), as a checked-in fixture.
3. **Minimal scorer** — precision + recall of produced edges vs ground truth.
   Reused verbatim in Phase 3.
4. **Baseline run** — count every restcall through the gate: `Enough` /
   `NeedsLLM` / `Junk`, with **`Junk` split `empty` vs
   `non-empty-but-failed-lexical-gate`** (the silent recall hole). Provisional:
   measured under the *old* lexical gate; re-taken after S1.4.
5. **Strong-vs-thin split** — using the signal extractor (1), how many residuals
   carry a strong class/import/acronym signal vs thin. Sizes the LLM tail.
6. **Gate thresholds (pre-commit)** — write them down so a cold session can act:
   - **GATE B:** deterministic tier resolves ≥ 60% of residuals → LLM is
     fallback-only; ≥ 90% → LLM deferrable.
   - **GATE C:** train-ticket precision < 0.7 → revisit candidate-narrowing
     (§6.4) before declaring done.
   (Numbers are starting points, revisable — but committed.)
7. **Optional, high-leverage and *not* new analysis:** extend the existing
   env_prefix reconciliation (`derive_constant_value_by_external_constants`)
   past the `settings.field` path, and resolve `${VAR}` placeholders the compose
   parser drops — both strengthen what symbolic eval *already* resolves.

**Files:** new signal extractor + scorer modules; ground-truth fixtures;
`pass3/restcalls.rs`, `pass3/env.rs`, `pass3/constants.rs` (read/measure).
**Output (all written to `sage_rework_results.md`):** ground truth + scorer in
the repo; the signal extractor validated; baseline buckets; the silent-recall
number; the strong-vs-thin split; committed gate thresholds.

### Phase 1 — Structural gate

Replace the lexical `url`/`uri` substring filter in
`is_restcall_evaluated_enough` with a **structural** decision. A `RestCall`
already *is* an HTTP call (it carries `http_method`); naming is irrelevant to
whether it is a real edge.

```
ResolvedURL      -> eval produced a concrete http... URL
NeedsResolution  -> eval left a non-empty, non-literal residual (ANY naming)
Junk             -> genuinely empty / non-HTTP / local-path noise only
```

This widens the population to all true residuals, removing the silent recall
hole, and lets Phase 2 do the discriminating.

**Files:** `pass3/restcalls.rs` (`EvalState`, `is_restcall_evaluated_enough`),
`llm_enhance/dispatch.rs` (filter on the new state).
**Output:** recovered `NeedsResolution` count vs the old `NeedsLLM` count.

### Phase 1.5 — Residual edge hygiene (added 2026-06-29)

**Why this exists.** The M1 preview (empaia, `--scrape`) showed the structural
gate recovered recall but also swept in **non-edges**: of 77 deterministic
matcher fires, ~74 landed on calls that are not cross-service HTTP at all. REST
calls are identified purely lexically (`python-extractor`:
`identification/method_call.rs` — *any* `*.get/post/...(arg)` whose first arg is
a `/`-string or a bare variable), so the `NeedsResolution` set is polluted with:

- **intra-service DB clients** — e.g. `ClassClient.get(class_id)` whose receiver
  is the class's own asyncpg-backed method (`self.get(class_id)`);
- **route-internal client reads** — `clients.annotation.get(annot_id=...)`;
- **dict/collection `.get`** with bare-variable keys.

FastAPI route *decorators* (`@app.get(...)`) are already excluded at
identification. The remainder is what we filter here.

**Discriminator (validated against empaia source).** A genuine cross-service
call's target is a **URL expression** — `self._http_client.get(self._mds_url +
url)` -> target `self._mds_url + url`. A non-edge's target is a **bare id /
param** — `class_id`, `annotation_id`, `item_id`. So classify a
`NeedsResolution` residual as a real cross-service residual iff its target has a
**URL nature**: an operand whose name hints url/uri/host/endpoint/base (reuse
`ranking.rs::name_hints_url`), or a `/`-path / `http` / a token matching a
config service host; otherwise it is a **non-edge**.

> **Tension to respect (do not repeat Phase 1's mistake).** Phase 1 deleted a
> lexical gate that over-pruned real URL-bearing calls by *variable naming*.
> This filter must NOT resurrect that: it junks calls that are *not URL-shaped
> at all* (DB/dict reads), not URL-bearing calls with an unlucky name. Keep the
> classification a **distinct `NonEdge` state**, never folded into `Junk`
> (empty), so what we drop stays auditable. Measurement-first: report the
> split before enforcing it.

**Plan:** measure the genuine-vs-non-edge split on empaia + train-ticket
(observation-only), then enforce by excluding `NonEdge` from the Phase 2 matcher
/ LLM. Receiver-type analysis (receiver is an httpx/requests/aiohttp client or
an injected `http_client`) is a sharper but heavier signal — defer unless the
URL-nature signal proves too coarse.

**Files:** new residual edge classifier under `llm_enhance/`; measurement via
`llm_enhance/baseline.rs`; enforcement later in the Phase 2 wiring.
**Output:** genuine cross-service residual count (the real Phase 2 population)
vs non-edge count, per corpus.

### Phase 2 — Service matcher

For each `NeedsResolution` residual, determine the target service, then rewrite
`target_uri` to that service's canonical URL.

Signal + candidate extraction (S2.0) is **already built in Phase 0**; Phase 2
consumes it.

**2a. Deterministic identifier→service** (rule-based, no LLM) — see §5.
High precision, deliberately partial recall; abstains on ambiguity.

**2b. LLM closed-set classification** (fallback for 2a's abstentions) —
"which of these N configured services does this call target?" Answer must be a
member of the config service set. Comprises:
- **2b-i. Query template** (`prompt.rs`) — see §6.
- **2b-ii. Answer contract** (`response.rs`) — parse a service id; validate by
  **membership + evidence grounding**; drop the self-reported-confidence gate.
- **2b-iii. Plumbing** (`query.rs`, `client.rs`, `query_builder.rs`,
  `dispatch.rs`) — new `QueryKind::ClassifyTargetService { candidates }`,
  structured-output request, outcome application.

**2c. Service→canonical-URL rewrite** — repurpose and fix
`rewrite_target_uri_with_resolution`: write `<service.urls[0]> + <resolved path
suffix>` instead of splicing free-text. Fix the `split('/')` bug. The path
suffix is best-effort; the synthesizer's fuzzy matching covers the remainder.

**Files:** new matcher module under `llm_enhance/`; `sage/src/resolver/{prompt,
query,response,client}.rs`; `llm_enhance/{query_builder,dispatch}.rs`;
`models/configuration.rs` (read access to service set).
**Output:** residuals resolved to canonical service URLs; LLM fired only on the
deterministic tier's abstentions.

### Phase 3 — Final evaluation + cleanup

- **Reuse the Phase 0 scorer** (already built) for the final precision + recall
  numbers on empaia *and* train-ticket. Extend ground truth to train-ticket if
  not done in Phase 0.
- **GATE C** on the train-ticket result (threshold from Phase 0).
- **Cleanup:** remove dead machinery — `ranking.rs::rank_and_cap` (lexical bag),
  the unused `QueryKind` variants, `FactBundle`'s snippet-only shape, the
  `variables_budget`, the prose constructor hint.

**Output:** trustworthy metrics on both corpora (recorded); lean codebase.

---

## 5. Deterministic identifier→service (Phase 2a) in detail

Lexical matching of the call site's identifiers against the config service set.
No LLM. Accepts only **unambiguous** matches; abstains otherwise.

### Inputs (descending signal strength)

1. **Enclosing client class** — the method's `Namespace::Class`
   (e.g. `MedicalDataServiceClient`), also the `source_span` class.
2. **File imports** — `from medical_data_service... import ...`
   (`file.imports` / `ImportGraph`).
3. **Operand identifiers** of `target_uri` — `self._mds_url`, `url`.

Candidates: `config.service_descriptions` (`name`, `urls`, `base_dir_path`).

### Normalization

Both sides reduce to a **token set** and an **acronym**, reusing the existing
identifier splitter in `ranking.rs` (Samurai / camel-snake):

```
service "medical-data-service"  -> tokens {medical,data,service}   acronym "mds"
service "clinical-data-service" -> tokens {clinical,data,service}  acronym "cds"

"self._mds_url"  -> strip self. , strip _url/_uri -> "mds"            (acronym key)
"MedicalDataServiceClient" -> splitCamel -> {medical,data,service,client}
import "medical_data_service" -> {medical,data,service}
```

**Generic tokens** (`service`, `client`, `url`, `uri`, `api`, `http`) are
stripped before matching, so they cannot cause spurious hits.

### Algorithm

```rust
fn deterministic_match(rc, ir, config) -> Option<Service> {
    let index = build_index(config.services); // tokens + acronym, generics stripped
    for signal in [client_class(rc, ir), imports(rc, ir), operand_idents(rc)] {
        let hits = index.match(normalize(signal)); // token-subset OR acronym equality
        match hits.len() {
            1 => return Some(hits[0]), // unambiguous -> accept
            0 => continue,             // try next signal
            _ => break,                // ambiguous -> abstain -> LLM
        }
    }
    None // no deterministic hit -> LLM
}
```

### Contract

High precision, deliberately partial recall. Accept only a unique match at the
strongest available signal; **abstain on ambiguity or no match** and defer to
the LLM. Purpose: skip an LLM round-trip (and hallucination risk) on the obvious
cases.

### Worked examples (empaia)

| Signal | Match | Result |
|---|---|---|
| class `MedicalDataServiceClient` | tokens == medical-data-service | ✅ unique → accept |
| `cds_url` | acronym `cds` → clinical-data-service | ✅ unique → accept |
| `es_url` | `es` → event-service **and** examination-service | ⚠️ ambiguous → LLM |
| `es_url` + class `ExaminationServiceClient` | class fires first, unique | ✅ accept |
| `base_url`, no telling class/import | none | → LLM (or unwinnable) |

Precedence ordering means the strong structural signal (class/import) usually
fires before the ambiguous variable-name acronym, so the deterministic tier
resolves more than the variable name alone would, and punts only the genuinely
ambiguous remainder.

### Per-corpus expectation

- **empaia** — distinctive names; high deterministic hit-rate.
- **train-ticket** — 40 × `ts-*-service`, acronyms collide heavily; the tier
  abstains often and leans on the distinctive middle token (`order`, `admin`,
  `info`). Lower hit-rate → more LLM fallback. The measurement that decides
  whether the deterministic tier earns its keep per corpus.

---

## 6. LLM query template rework (Phase 2b-i) in detail

`prompt.rs` is built for the old job and almost none survives. The model stops
*generating* a URL and starts *choosing* from a closed set. What changes, at a
glance: `build_system_message` becomes the closed-set contract below;
`build_question_message`'s six `QueryKind` variants collapse to one
`ClassifyTargetService` template; `build_facts_message`'s whole-class snippet
becomes the curated, precedence-ordered signals; `build_variables_message` (flat
150-entry bag + prose hint) is **deleted**; and the request is paired with an
Ollama `format`/grammar that constrains `service` to the candidate enum.

### 6.1 System message — the closed-set contract

```
You are a software-architecture analysis assistant. A REST client in one
microservice calls another microservice over HTTP. Given a call site whose
target URL could NOT be resolved statically, identify which microservice it
targets.

Choose exactly one service from CANDIDATE SERVICES, or null if none clearly
matches. Respond with a single JSON object and nothing else:

{
  "service": "<exact name from CANDIDATE SERVICES, or null>",
  "evidence": ["<token from CONTEXT that justifies the choice>", ...],
  "reasoning": "<one short sentence>"
}

Rules:
- "service" MUST be copied verbatim from CANDIDATE SERVICES, or be null.
- Do NOT invent a service. Do NOT output a URL or a value.
- "evidence" must cite concrete tokens present in CONTEXT (class, variable,
  import). An answer with no grounding token is invalid.
- No text outside the JSON. No markdown.
```

### 6.2 Task + context message — curated signals, precedence-ordered

Not the enclosing-class blob. The signals are labeled and ordered strongest
first, matching the Phase 2a precedence.

```
Which microservice does this call target?

ORIGIN SERVICE: app-service        (exclude as the answer; this is the caller)

CALL SITE
  client class : MedicalDataServiceClient      <- strongest signal
  imports      : from ....custom_clients.mds_client import MedicalDataServiceClient
  expression   : self._mds_url + url
  identifiers  : self._mds_url, url            <- weakest signal

CANDIDATE SERVICES
  - medical-data-service    (http://medical-data-service:8000)
  - clinical-data-service   (http://clinical-data-service:8000)
  - examination-service     (http://examination-service:8000)
  - job-service             (http://job-service:8000)
  ... (one line per configured service)
```

### 6.3 Structured-output schema (Ollama `format`)

Constrains the decode so an out-of-set name cannot be produced — this is what
eliminates the 53%-invalid-JSON failure mode at the source.

```json
{
  "type": "object",
  "properties": {
    "service":   { "type": ["string", "null"], "enum": [<candidate names...>, null] },
    "evidence":  { "type": "array", "items": { "type": "string" } },
    "reasoning": { "type": "string" }
  },
  "required": ["service", "evidence"]
}
```

### 6.4 Design points

- **Signal ordering is load-bearing.** `client class` first, `identifiers` last,
  nudges the model toward the structural signal — the same precedence Phase 2a
  uses. This is what makes the `self._mds_url` cases easy: the class name is in
  scope.
- **ORIGIN SERVICE is passed and excluded** — lets the model and the validator
  reject self-loops.
- **Candidate list size.** empaia (~20): list all. train-ticket (~40 `ts-*`):
  consider pre-narrowing to Phase 2a's top-k token matches so the model
  disambiguates a short list, not 40 — but that couples 2a and 2b. Default: list
  all; add narrowing only if train-ticket precision demands it.

Coupling to 2b-ii (`response.rs`): the validator enforces `service ∈
candidates ∪ {null}` (redundant with the enum, but cheap) and that each
`evidence` token appears in the context string (grounding).

---

## 7. Open decisions

1. **No new static analysis (decided).** Residuals eval cannot close —
   including `self.`-attribute targets — are resolved to a *service* by Phase 2,
   not to a *value* by a new pass. The only env work permitted is extending
   reconciliation/placeholder handling that strengthens what eval *already*
   does. Revisit only if the train-ticket measurement fails.
2. **Is the LLM even needed?** After Phases 0–2a, measure the remaining residual
   tail on empaia. If the deterministic class/acronym tier closes most of it,
   the LLM becomes a small, possibly deferrable fallback.
3. **Path suffix fidelity** — Phase 2c may recover only the service base URL when
   the path is a caller-supplied parameter. Confirm the synthesizer's fuzzy
   endpoint match is sufficient on base-URL-only targets (expected: yes, since
   SDG attribution is service-level).

---

## 8. Explicitly out of scope

- **The synthesizer** — unchanged; consumes URL `target_uri`s as today.
- **Any new static analysis for residuals** — decided against. No constructor-
  injection pass, no cross-file def-use slicer, and the LLM is **not** asked to
  generate URL values. `self.`-attribute and other unresolved targets are
  classified to a *service* (Phase 2). The redesign's Reframe 3/4 remain a
  last-resort fallback only if the train-ticket measurement shows classification
  is insufficient.
- **Secrets redaction** — intentionally absent (self-hosted LLM, no exfiltration
  boundary), per `sage_validation_findings.md`.

---

## 9. Implementation steps

Dependency-ordered. The build front-loads measurement and ships the
deterministic path before the LLM, so the "is the LLM even needed" decision is
data-driven. Three gates (**A/B/C**) punctuate the sequence. **Every step obeys
the per-phase rule (§4): end green, committed, with results written to
`docs/sage_rework_results.md`.**

### Phase 0 — Foundation + baseline

- **S0.1 — Signal extractor (= S2.0, pulled forward).** From `RestCall` +
  `ProjectIR` + `ConfigurationData`: origin service, client class, file imports,
  operand identifiers, candidate services. *Files:* new `llm_enhance/signals.rs`.
  *Validate* on real empaia residuals.
- **S0.2 — Ground truth + scorer.** *Done, but demoted (2026-06-29).*
  Auto-derived oracle (`llm_enhance/oracle.rs`) joins `empaia-constants.json`
  (identifier -> URL) with `empaia-config.json` (URL -> service) on host;
  precision/recall scorer (`llm_enhance/scorer.rs`). **Demotion:** empaia has no
  published dependency graph and hand-labeling is out, so the constants file is
  the only automatic labeler — but it is *partial* (a residual is only scoreable
  when its leftover operand is a `*_url` constant name). On the real `--scrape`
  population only **11/152** residuals are scoreable; the curated-vs-scrape delta
  caps it at ~24. So the oracle is a **small precision spot-check, not a
  headline metric**, and is **not** a trustworthy basis for GATE B/C on empaia.
  It is NOT a circular check (matcher resolves from names; oracle from URL
  values). Kept as-is; do not invest in widening it. *Files:* `oracle.rs`,
  `scorer.rs`.
- **S0.3 — Baseline run.** Count buckets through the gate: `Enough` / `NeedsLLM`
  / `Junk` (split `empty` vs `non-empty-but-failed-lexical-gate`). Provisional
  (old lexical gate); re-taken at S1.4. *Files:* `pass3/restcalls.rs`,
  `dispatch.rs` (temporary counting). *Output:* silent-recall number; residual
  set.
- **S0.4 — Strong-vs-thin split.** Run S0.1's extractor over the residuals;
  classify strong (class/import/acronym) vs thin. *Output:* the LLM tail size.
- **S0.5 — Commit gate thresholds** to `sage_rework_results.md`: GATE B
  (deterministic ≥60% → LLM fallback-only; ≥90% → deferrable); GATE C
  (train-ticket precision <0.7 → revisit narrowing).
- **GATE A:** confirms the gate is the leak; sizes Phases 1–2.

### Phase 1 — Structural gate

- **S1.1 — Rework the gate.** `EvalState` → `ResolvedURL | NeedsResolution |
  Junk`; drop the `url`/`uri` substring test for a structural rule (non-empty,
  non-literal residual on an HTTP call → `NeedsResolution`). *Files:*
  `pass3/restcalls.rs`.
- **S1.2 — Update the filter** in `dispatch.rs` to select `NeedsResolution`.
- **S1.3 — Tests** for the new bucketing.
- **S1.4 — Re-run S0.3 + S0.4** under the new gate; compare `NeedsResolution`
  vs old `NeedsLLM`, and refresh the strong-vs-thin split on the real
  population.

### Phase 1.5 — Residual edge hygiene (added 2026-06-29; see §4)

- **S1.5 — Residual edge classifier.** Partition the `NeedsResolution` set into
  genuine cross-service residuals vs `NonEdge` (DB/dict/intra-service `.get`
  noise) by **target URL-nature** (operand hints url/uri/host/endpoint/base via
  `ranking.rs::name_hints_url`, or `/`-path / `http` / config-service-host
  token). Keep `NonEdge` a distinct state — never folded into `Junk` (empty).
  *Files:* new classifier under `llm_enhance/`. *Measurement-first* (observe the
  split; do not yet change what the matcher sees).
- **S1.6 — Measure the split** on empaia + train-ticket via `baseline.rs`;
  record the genuine-vs-non-edge counts. This is the real Phase 2 population.
- **S1.7 — Enforce** (after the split is reviewed): exclude `NonEdge` from the
  Phase 2 matcher / LLM. *Files:* the Phase 2 wiring (`query_builder.rs` /
  `dispatch.rs`).

### Phase 2 — Service matcher

- **S2.0 — (done in Phase 0, S0.1)** signal + candidate extraction. Phase 2
  consumes it; if Phase 0 stubbed any signal, complete it here.
- **S2.1 — Deterministic matcher (2a).** Service index (tokens + acronym,
  generics stripped) reusing `ranking.rs`'s splitter; match-by-precedence;
  abstain on ambiguity/none → `Option<Service>`. *Files:* new `matcher.rs`.
  *Tests:* the §5 worked examples.
- **S2.2 — Service→canonical-URL rewrite (2c).** Fix/repurpose
  `rewrite_target_uri_with_resolution` to write `<service.urls[0]> + <suffix>`;
  kill the `split('/')` bug. *Files:* `query_builder.rs`.
- **S2.3 — Wire deterministic-only path.** `query_builder` runs S2.0→S2.1; on
  hit, rewrite via S2.2; on abstain, leave residual untouched. *Files:*
  `query_builder.rs`, `dispatch.rs`.
- **MILESTONE M1 — measure deterministic coverage** on empaia + train-ticket.
- **GATE B (Open Decision 2):** if deterministic closes most residuals, the LLM
  is a small fallback — proceed; if enough, optionally stop.
- **S2.4 — LLM plumbing (2b-iii).** `QueryKind::ClassifyTargetService {
  candidates }`; `SageQuery` carries S2.0 signals (retire
  `FactBundle`/`variables_map`); `client.rs` sends the Ollama `format` enum
  schema (§6.3). *Files:* `sage/query.rs`, `sage/client.rs`.
- **S2.5 — Query template (2b-i).** Rewrite `prompt.rs` to §6.1/6.2; delete
  `build_variables_message`; collapse the six question kinds. *Files:*
  `sage/prompt.rs`.
- **S2.6 — Answer contract (2b-ii).** `SageResponse { service, evidence,
  reasoning }`; `validate()` = membership + grounding; drop the confidence gate;
  update `SageError`. *Files:* `sage/response.rs`.
- **S2.7 — Wire LLM fallback.** `dispatch.rs`: deterministic first, LLM for
  abstentions, then rewrite from the chosen service. *Files:* `dispatch.rs`,
  `query_builder.rs`. *Tests:* orchestration + rewrite.

### Phase 3 — Evaluation + cleanup

- **S3.1 — Reuse the Phase 0 scorer.** Extend ground truth to train-ticket if
  not already done.
- **S3.2 — Run empaia + train-ticket;** record metrics to
  `sage_rework_results.md`.
- **GATE C (§8 fallback):** if train-ticket precision is poor → revisit
  candidate-narrowing (§6.4) or a value path; else done.
- **S3.3 — Cleanup.** Remove `ranking.rs::rank_and_cap`, dead `QueryKind`s,
  `FactBundle` snippet shape, `variables_budget`, prose hint, orphaned
  `build_snippet`.
- **S3.4 — Update docs** with final metrics.

### Critical path & parallelism

```
S0 (signals + ground truth + scorer + baseline) → GATE A
   → S1 → S2.1 ─┬─ S2.2 → S2.3 → [M1 / GATE B]
                └─ (S2.4 ∥ S2.5 ∥ S2.6) → S2.7 → S3 → GATE C
```

- S2.0 is built in Phase 0 (S0.1); S2.1 consumes it.
- S2.4 / S2.5 / S2.6 are independent once `QueryKind` exists — parallelizable.
- Gates: **A** (gate is the leak), **B** (is the LLM needed), **C** (train-ticket
  verdict).
- Every node ends green + committed + recorded (§4 rule).
