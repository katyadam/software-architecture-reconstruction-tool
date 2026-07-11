# Sage Rework — Consolidated Knowledge Report

Everything known about the sage REST-call target-resolution rework as of
2026-07-02: history, architecture, measured results, design rationale,
evaluation critique, and proposed future work. Written as the single
catch-up document for a cold reader (or a paper draft).

- **Branch:** `sage-rework` (off `llm-sage`)
- **Companion docs:** [`sage_rework_plan.md`](sage_rework_plan.md) (the plan),
  [`sage_rework_results.md`](sage_rework_results.md) (measurement log),
  [`sage_resolution_redesign.md`](sage_resolution_redesign.md) (diagnosis),
  [`sage_validation_findings.md`](sage_validation_findings.md) (first
  validation).

---

## 1. Context — what the tool is and where sage fits

VOYANTCLAIR is a Software Architecture Reconstruction (SAR) tool for
distributed Java/Python codebases. It extracts code elements (endpoints, REST
calls, constants) per microservice, then synthesizes architectural views:
Context Map, System Dependency Graph (SDG), Inter-Microservice Call Graph
(IMCG).

The weak link in SAR is the **REST-call edge**: a call site's target URL is
often not a literal but an expression over env vars, injected config, or
class attributes (`self._mds_url + url`). Symbolic evaluation plus scraped
config resolves most; the **residual** set is where edges go missing. The
`sage` crate is an LLM arbiter for that residual tail — always a fallback,
never the primary engine.

Fixed architectural boundary: the extractor must hand the synthesizer a
URL-shaped `target_uri`. Whatever resolution decides — even when it decides a
*service* — is written back as that service's canonical config URL (plus
best-effort path suffix). The synthesizer is untouched.

## 2. History — why the rework exists

First end-to-end validation (2026-06-06, branch `llm-sage`, empaia corpus,
qwen2.5-coder:7b via local Ollama, open-ended URL *generation*):

**271 queries, 0 correct resolutions, 0 SDG connections. Precision 0%.**

- 53% invalid JSON (no structured-output enforcement despite the design
  calling for it).
- Accepted answers were all garbage: `example.com`, template placeholders,
  echoed variable names, localhost. The real URL never reached the prompt, and
  the prompt invited guessing over abstaining.
- Buggy lookup-key derivation in `query_builder.rs` (`self._mds_urlurl`).

Diagnosis (see `sage_resolution_redesign.md`): ingestion was *not* the
blocker — config values were already scraped. The blockers were consumption
(available values not applied), an over-pruning lexical gate, and the wrong
question asked of the LLM. Hence the rework thesis: widen the residual
population, resolve the service deterministically where possible, and ask the
LLM a **closed-set classification** question only for the ambiguous tail.

## 3. Target architecture

```
symbolic eval + generate_uris        (existing deterministic URL resolution)
  -> structural gate                 ResolvedURL | NeedsResolution | Junk(empty)
       -> residual edge hygiene      CrossService | NonEdge
            -> deterministic matcher identifier -> service (abstains on ambiguity)
                 -> LLM closed-set   "which of these N services?" (fallback)
                      -> rewrite     target_uri = service canonical URL + suffix
synthesizer (UNCHANGED)              URL -> endpoint -> service -> SDG/IMCG/CM
```

Unified triage lives in `llm_enhance/residual_edge_filter.rs::triage` ->
`ResidualTriage { Resolved | Empty | NonEdge | NeedsResolution }`. `Empty` and
`NonEdge` stay distinct drop reasons so every dropped call is auditable.

## 4. What is built and measured (phase by phase)

### Phase 0 — measurement scaffolding
- Signal extractor (`llm_enhance/signals.rs`): origin service, client class,
  imports, operand identifiers. Key discovery: the global
  `project_ir.callable_map` is keyed by mangled name only (no hash); class
  recovery needs the file-local scan by `metadata.hash`.
- Auto-derived oracle (`oracle.rs`) + scorer (`scorer.rs`): joins curated
  constants (identifier -> URL) with config (URL -> service) on **host**.
  Non-circular (matcher resolves from names, oracle from URL values). Later
  **demoted to spot-check**: scores only 11/152 residuals (~7%), ceiling ~24.
- Baseline buckets under the old lexical gate: empaia forwarded 18 residuals
  to the resolver while silently junking **110** non-empty ones;
  train-ticket 2 vs 22. **GATE A confirmed: the lexical gate was the leak.**

### Phase 1 — structural gate
Replaced the `url`/`uri` substring test with a structural rule: empty ->
`Junk`; starts with `http` -> `ResolvedURL`; any other non-empty residual ->
`NeedsResolution`. Recovery: empaia 18 -> 128 (~7x), train-ticket 2 -> 24
(12x). `Junk:non-empty` is 0 by construction — the silent recall hole is
closed.

### Phase 1.5 — residual edge hygiene
The M1 preview showed the widened population is polluted: REST-call
identification is purely lexical (`python-extractor`: any `*.get/post(arg)`),
so `NeedsResolution` swept in intra-service DB clients
(`ClassClient.get(class_id)`, asyncpg-backed), route-internal reads, and dict
`.get`. Confirmed false edge: `JobClient -> job-service` (a DB client).

Classifier (`residual_edge_filter.rs`): `CrossService` iff target contains
`http`, contains `/`, or any operand identifier hints URL-nature
(`URL/URI/HOST/ENDPOINT/BASE/PORT`). A 4th rule (operand token matches a
service name) was implemented and **removed**: domain nouns *are* the service
names (`annotation_id` vs `annotation-service`), so it inverted the split.

Measured (empaia, `--scrape`): 152 residuals -> **43 cross-service (28%) /
109 non-edge (71%)**. Enforced in S1.7: dispatch triages before resolution;
LLM population dropped 152 -> 43.

### Phase 2a — deterministic matcher
`llm_enhance/matcher.rs`: services indexed as token sets + acronyms (generics
`service/client/url/uri/api/http` stripped). Signals tried strongest-first
(client class, imports, operand identifiers); within a group, origin service
excluded; exactly one hit -> accept, zero -> next signal, >1 -> abstain to
LLM. Plus the service -> canonical-URL rewrite (`query_builder.rs`,
`split('/')` bug fixed) and dispatch wiring: deterministic pass rewrites
`target_uri` in place; resolved calls read as `ResolvedURL` and skip the LLM
batch automatically.

### M1 / GATE B (empaia `--scrape`, 2026-06-29)

| metric | value |
|---|---|
| total restcalls | 577 |
| ResolvedURL (symbolic eval) | 425 (73%) |
| NeedsResolution | 152 (26%) |
| cross-service after hygiene | 43 |
| matcher hits | 26/43 (60%) |
| actually resolved (rewritten) | **20/43 (47%)** |
| LLM fallback population | 23 |

The 26-vs-20 gap: 6 hits land on the config's `models` entry — a shared
data-models package (`urls: []`), not a microservice; they abstain on no-URL.
Honest resolution rate 47% < the 60% GATE B bar -> **LLM is needed as a
fallback**. Flagged cheap follow-up (open): exclude no-URL non-services from
the matcher index and re-measure; it can only unmask shadowed second-place
matches, so it can only raise resolution.

### Remaining work
- Phase 2b — LLM closed-set classifier for the 23 abstentions:
  `QueryKind::ClassifyTargetService { candidates }` plumbing (S2.4, candidates
  must exclude origin), prompt rework (S2.5), answer contract = membership +
  evidence grounding, confidence gate dropped (S2.6), dispatch wiring (S2.7).
  Ollama `format` enum schema constrains the decode so an out-of-set answer
  cannot be produced — this removes the 53%-invalid-JSON mode at the source.
- Phase 3 / GATE C — final precision/recall on empaia + train-ticket; cleanup
  (`ranking.rs::rank_and_cap`, dead `QueryKind`s, `baseline.rs`,
  `variables_budget`).
- Uncommitted at time of writing: S2.2/S2.3 changes in `dispatch.rs`,
  `query_builder.rs`, and the two planning docs.

## 5. Load-bearing design decisions

1. **No new static analysis for residuals.** No constructor-injection pass,
   no cross-file def-use slicer. Rationale: the strongest classification
   signal (the client class name) is already in scope at the residual, so a
   value pass would be largely redundant. Revisit only if train-ticket fails
   GATE C.
2. **Closed-set over generation.** The LLM chooses from configured services;
   it never produces a URL or value. Grounded in the 0%-precision run.
3. **Abstention is a first-class outcome** at every tier: matcher abstains on
   ambiguity; LLM may answer null; no-URL match = no-op abstain.
4. **Drop reasons stay auditable.** `Empty` vs `NonEdge` never folded
   together; enforcement only after the split is measured and reviewed.
5. **Repo is the only durable memory.** Every phase ends green, committed,
   and recorded in `sage_rework_results.md`.
6. **Oracle demoted, not deleted.** Reliable where present (URL values are
   deployment fact) but too partial (~7%) to gate on.

## 6. Assessment — is the plan/evaluation right?

### What is right
- The closed-set reframe, constrained decoding, and evidence grounding each
  target a *measured* failure mode from the June run. This is
  failure-driven design, not speculation.
- Deterministic-first with LLM-on-abstain minimizes LLM exposure (23 call
  sites), keeps the bulk path reproducible, and makes the "is the LLM even
  needed" question empirical (GATE B answered it: yes, as fallback).
- Measurement-before-enforcement caught two things a build-first approach
  would have shipped: the silent recall hole (110 junked residuals) and the
  71% non-edge pollution.

### Gaps that matter for publication (threats to validity)

1. **Ground truth is the weakest link.** All headline numbers so far are
   *coverage*, not *correctness*; the only precision signal is 2/3 on an
   11-sample oracle. Hand-labeling was ruled out when the population was
   unbounded — but the final population is **43 (empaia) + 24 (train-ticket)
   = 67 residuals**. Hand-label all of them. Hours of work; converts every
   coverage number into a real precision/recall number. Without it, GATE B/C
   verdicts are not defensible in review.
2. **Hygiene-filter recall is unmeasured.** Rules 1-3 are lexical; a genuine
   cross-service call whose operand is blandly named (`path`, `target`) is
   silently dropped into the 109. Audit a random sample (~30) of the dropped
   `NonEdge`s for false drops. Also note the known over-match coarseness
   (`BASE` hits `database`, `PORT` hits `report`).
3. **Training-data contamination.** Train-ticket is a canonical benchmark;
   its architecture is almost certainly in the LLM's training data. A correct
   closed-set answer may come from memorization, not from the presented
   signals. Control: re-run with service names anonymized to opaque tokens
   (`ts-order-service` -> `svc-17`) in both candidates and context; report
   both conditions. Empaia is obscure — the contrast between corpora is
   itself a finding.
4. **Nondeterminism.** Fix temperature = 0 and a seed; run the LLM tier N>=5
   times; report agreement/variance. Cheap on 23 queries.
5. **No ablations yet.** A paper wants: (a) pipeline stages — old gate vs
   structural gate vs +hygiene vs +matcher vs +LLM; (b) signal precedence —
   class-only vs +imports vs +operands; (c) model sensitivity — small coder
   model vs larger local vs a frontier API as ceiling; (d) candidate
   narrowing on train-ticket (all 40 vs top-k).
6. **No end-to-end metric.** Call-site resolution is an intermediate. The
   claim that matters is SDG-edge precision/recall after the synthesizer's
   fuzzy matching (which can both absorb and introduce error). Train-ticket
   has published dependency ground truth in prior SAR literature (Cerny et
   al.'s group) — evaluate the final SDG against it, and position against
   existing SAR extractors (e.g. Code2DFD-class tools) as baselines. The June
   0%-precision generation run is a built-in ablation baseline — report it.
7. **Two corpora is thin.** Both are needed but neither alone generalizes:
   empaia (Python, distinctive names) and train-ticket (Java, 40 colliding
   `ts-*` names) are near-opposite regimes, which is good — but adding 1-2
   Java Spring benchmarks (e.g. piggymetrics, spring-petclinic-microservices,
   lakeside-mutual) would strengthen external validity at modest cost.
8. **Gate thresholds are engineering devices, not results.** 60%/90% (GATE B)
   and 0.7 (GATE C) are committed decision rules — fine for execution
   discipline, but the paper should report the full numbers and curves, not
   the gate verdicts.
9. **The Java residual population is partly an extraction artifact, and it
   interacts with a validation gap.** (Discovered in the 2026-07-11 Phase 3
   train-ticket run; full trace in `sage_rework_results.md` §Phase 3.)
   **STATUS: the artifact is FIXED (2026-07-11)** — the `pass3/restcalls.rs`
   `Err` branch now resolves pure literals env-free, dropping train-ticket's LLM
   tail from 22 to 2 and eliminating the finding-3 error class at the source. The
   analysis below is retained because it is the paper's story (artifact → wrong
   population → downstream LLM error) and because the *validation gap* and the
   *test-file scoping* question remain open.
   Java string-literal call URLs retain their **surrounding double-quotes**
   through symbolic evaluation (the Java *calls* extractor never strips them —
   only the *endpoints* extractor does), so a fully-known target such as
   `"http://ts-price-service:16579/api/v1/priceservice/prices"` has a
   `target_uri` beginning with `"`, fails the gate's `target_uri.starts_with(
   "http")` test (`pass3/restcalls.rs`), and is misclassified `NeedsResolution`
   even though nothing is actually unresolved. Two consequences the paper must
   control for:
   - **Population contamination.** Most of train-ticket's 22 cross-service
     "residuals" are these quote-wrapped literals (several also in `src/test/`
     files) — not genuine unresolved calls. They inflate the LLM-tier
     denominator and depress the apparent *static*-resolution recall on Java.
     The honest protocol is to strip the quotes (fold literal URLs to
     `ResolvedURL`), re-measure, and report the residual set that actually
     survives — the true unresolved population is much smaller than the raw
     count suggests. This is a corpus/pipeline artifact, not a property of the
     resolver.
   - **Grounded ≠ correct (a real closed-set failure mode).** Because these
     resolved-but-mislabeled URLs reach the LLM, the classifier gets a chance
     to err on inputs it should never see. For the `ts-price-service` URL the
     model chose `ts-consign-price-service` (both are candidates) — in one of
     the two cases *quoting `ts-price-service` in its own evidence* while
     picking the other, reasoning it "aligns closest." The evidence-grounding
     check passes (the cited token IS in the context) yet the choice
     contradicts the evidence: `validate()` enforces token-in-context, not
     token-supports-the-chosen-service. This is a concrete instance motivating
     (a) candidate narrowing among near-duplicate names (§6.4) and (b) a
     stricter validation rule (the chosen service's name-tokens must appear in
     the cited evidence). 2/18 train-ticket choices failed this way. Report it
     as a named failure mode, not just an error count.
   - **Mechanism — associative, not lexical, matching.** The model is *not*
     string-matching the URL host against the candidate list. It reasons
     associatively: it sees a "price" endpoint and, among 45 candidates
     containing two price services, picks the longer / more specific-sounding
     `ts-consign-price-service`. In the second case it quotes `ts-price-service`
     in its own evidence yet still chooses the other, calling it "aligns
     closest" — choice contradicting evidence. This is characteristic of a small
     coder model (qwen2.5-coder:7b) and predicts that (i) it worsens as
     candidate sets grow and contain near-synonyms, and (ii) it would likely
     vanish with a frontier model — making *model capacity* an ablation axis,
     not a fixed condition.
   - **Why structured output does not prevent it.** The `response_format`
     json-schema enum constrains the answer to the *set* of candidate names
     (and here `strict:false`, so Ollama does not even hard-enforce the set —
     `validate()` does). It cannot express "the service whose name appears in
     the URL," so both `ts-price-service` and `ts-consign-price-service` are
     equally legal outputs. Structured output eliminates the *invalid-JSON /
     hallucinated-non-candidate* failure mode (its actual contribution: 0/40
     invalid across both corpora vs 53% in the June generation baseline) but is
     orthogonal to *which* legal candidate gets chosen. The paper should draw
     this line explicitly: schema constraints fix well-formedness and set
     membership, not semantic correctness of the selection.

### Recommended evaluation protocol for the paper

1. Hand-label all 67 cross-service residuals (target service or
   "unresolvable") + a 30-sample audit of dropped non-edges.
2. Report per corpus: population funnel (total -> resolved-by-eval ->
   residual -> cross-service), then precision/recall per tier (deterministic
   alone, +LLM), and end-to-end SDG-edge P/R vs reference.
3. LLM tier: temperature 0, fixed seed, 5 runs, both plain and anonymized
   service names, 2-3 models.
4. Ablations from §6.5 above.
5. Report abstention rates and false-edge counts explicitly — the
   architecture's claim is precision-first, so show what it *refuses* to
   answer, not only what it gets right.

## 7. Future work and research directions

### Direct extensions
- **Constructor-injection / cross-file value pass** — the deliberately
  deferred static alternative. Research angle: measure its marginal value
  *against* the LLM tier (static-vs-LLM cost/precision trade on the same
  residuals). Only justified if GATE C fails, but as a paper it is a clean
  controlled comparison.
- **Candidate narrowing for large service sets** — embedding- or
  token-based top-k pre-narrowing for 40+ service corpora; measures how
  closed-set accuracy degrades with candidate-set size.
- **Config-source generalization** — ingest Kubernetes manifests, Helm
  values, and service-discovery registrations (Eureka/Consul) as additional
  `external_constants` sources; today only `.env*` and docker-compose are
  scraped.
- **Language coverage** — Go and TypeScript extractors; both are dominant in
  microservice codebases and have the same residual-URL problem.

### New edge types (bigger SAR gap than REST)
- **Asynchronous messaging edges** — Kafka/RabbitMQ/NATS topic publish/
  subscribe matching. Topics are string constants suffering the same
  residual-resolution problem, and async edges are invisible to URL-based
  SAR entirely. The gate -> hygiene -> deterministic -> closed-set-LLM
  pipeline transfers almost verbatim (topic instead of URL, consumer-group
  instead of endpoint).
- **gRPC / GraphQL** — proto-service and schema-based matching; mostly
  deterministic, useful to show the pipeline's non-LLM tiers generalize.

### Evaluation-as-contribution
- **Publish the labeled residual benchmark.** A curated corpus of
  hard-to-resolve call sites (expression, signals, ground-truth service)
  across empaia + train-ticket (+ any added corpora) is a reusable dataset —
  SAR papers chronically lack exactly this.
- **Contamination-controlled SAR evaluation** — the anonymization protocol
  from §6.3 generalizes: a methodology paper on evaluating LLM-assisted
  program analysis on public benchmarks the model has memorized.

### Dynamic and longitudinal
- **Runtime cross-validation** — deploy the corpus via docker-compose, drive
  traffic, capture actual service-to-service calls (eBPF or mesh sidecar),
  and diff against the static SDG. Gives (a) automatic ground truth and (b) a
  static-vs-dynamic SAR completeness study.
- **Architecture drift / conformance in CI** — run reconstruction per commit,
  diff SDGs over history, flag erosion (new cycles, unauthorized edges)
  against a declared target architecture. The manager/commit infrastructure
  already stores per-commit metadata.
- **Antipattern detection on reconstructed views** — cyclic dependencies,
  hub/god services, chatty pairs, nano-services — detectors over the IMCG,
  turning the views into actionable findings.

### Human-in-the-loop
- **Uncertainty-aware SDG** — every arbiter edge already carries provenance
  (deterministic vs LLM, evidence tokens). Surface that in the views: solid
  vs dashed edges, click-through to the call site and the LLM's evidence.
  Architect confirms/rejects; confirmations feed back as labels.
- **LLM-assisted labeling** — use a frontier model to *propose* ground-truth
  labels for new corpora, human-verify, then evaluate the local-model
  pipeline against them; makes multi-corpus evaluation affordable.

## 8. Pointers

| What | Where |
|---|---|
| Plan | `docs/sage_rework_plan.md` |
| Results log (durable memory) | `docs/sage_rework_results.md` |
| Triage + hygiene filter | `extractor-runtime/src/pipeline/pass3/llm_enhance/residual_edge_filter.rs` |
| Signals | `.../llm_enhance/signals.rs` |
| Deterministic matcher | `.../llm_enhance/matcher.rs` |
| Oracle / scorer (spot-check only) | `.../llm_enhance/{oracle,scorer}.rs` |
| Rewrite + dispatch wiring | `.../llm_enhance/{query_builder,dispatch}.rs` |
| LLM client/prompt/contract (Phase 2b target) | `sage/src/resolver/{prompt,query,response,client}.rs` |
| Temporary measurement (removed in S3.3) | `.../llm_enhance/baseline.rs` |
