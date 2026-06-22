# Sage Arbiter — Resolution Redesign

Architectural critique of the current LLM resolution approach on the `llm-sage`
branch, and a proposed redesign. Companion to [`sage_validation_findings.md`](sage_validation_findings.md)
(empirical 0% precision run) and [`llm_sage_branch_summary.md`](llm_sage_branch_summary.md).

- **Date:** 2026-06-15 (rev. 2026-06-22 — corrected the snippet-size claim; see §1/§2.1)
- **Scope:** *what* sage sends to the model and *what* it asks, not prompt/JSON tuning.

---

## 1. What the current design actually does

Per residual REST call (`extractor-runtime/.../pass3/llm_enhance/`), the prompt is:

- **One large snippet** — the bytes of `source_span`, which the extractors set
  to the *enclosing class* (Python `last_class_last_function_span`, Java
  `last_class_first_function_span`), falling back to the enclosing
  function/method, then the whole file. `build_snippet` therefore ships the
  entire class containing the call, not the `self._mds_url + url` expression
  alone.
- **A flat bag** of up to `variables_budget` (150) entries
  `[microservice|file|scope|name] = value`, ranked by lexical name-similarity
  (`ranking.rs::rank_and_cap`).
- **One fixed question** — always `QueryKind::ResolveLookup`, with a key derived
  by `rc.target_uri.split('/').next()` (`query_builder.rs`).
- A prose hint asking the model to follow constructor injection
  (`prompt.rs::build_variables_message`).

In short: retrieval-augmented generation where retrieval = bag-of-variables, and
the unit of context is the whole enclosing class — which carries the call's
*intra-class* definitions but nothing across the file boundary.

---

## 2. Why it scores 0%, structurally

`sage_validation_findings.md` blames JSON enforcement (#1), self-reported
confidence (#2/#8), and key mangling (#3). Those are real but **secondary**.
Even with perfect JSON and a perfect validator, finding #2 is fatal: *the answer
is not in the prompt*. You cannot validate a correct answer into existence.

Root causes, in order of importance:

1. **The chain breaks at the class boundary, not below the statement.** The
   snippet is the whole enclosing class, so the call's *intra-class* links are
   present: the model can see `self._mds_url = base_url` in `__init__`. What is
   absent is everything across the file boundary — the **construction site**
   (`MedicalDataServiceClient(settings.mds)`, in the caller's class) and the
   **config/env value** (`mds = "..."`). The data-flow slice is severed exactly
   where it leaves the class. So the retrieval gap is cross-file, and a slicer's
   job is to *bridge that boundary*, not to reconstruct what the class already
   shows.

2. **Ranking optimizes the wrong axis — over a noisy blob.** Lexical
   name-similarity, computed against a whole-class (or whole-file fallback)
   snippet. The missing link is *structural* — `self._mds_url` flows through a
   constructor parameter likely named `base_url`, which has **zero lexical
   overlap** with `_mds_url`. More budget or better ranking will not help: we
   tune recall on an axis the answer does not live on, and the large blob only
   dilutes the lexical signal further.

3. **The pipeline assumes URL-shaped residuals.** `lookup_key` and
   `rewrite_target_uri_with_resolution` both `split('/')` on `target_uri` — but
   residuals are *expressions* (`self._mds_url + url`), never URLs. Both functions
   operate on the wrong kind of string. Hence keys like
   `ResolveLookup:self._mds_urlurl`.

4. **We built a static analyzer, then discarded the static analysis at the LLM
   boundary.** The extractor already holds constants, assignments, attr maps,
   per-file module consts, and transitive imports. The constructor-injection case
   is *literally the prompt hint text* — we know the pattern, but we solve it by
   asking the model nicely in prose instead of resolving the constructor edge in
   code and handing over the result.

---

## 3. The precondition nobody can skip

Before redesigning anything: **audit how many of the 271 residual sites are
resolvable from ingested facts at all.**

If `settings.mds` ultimately comes from a docker-compose file, a k8s manifest, or
a `.env` variant the scraper skips (finding #7), then no LLM and no slicer can
resolve it — the value is not in the corpus. So:

- Ingest config sources first: docker-compose, k8s manifests, `.env` / `*.env`
  variants (`sample.env`, `development.env`, ...).
- Re-measure. If most residuals are unwinnable, the fix is **ingestion**, not the
  arbiter.

---

## 4. Four reframes, most leverage first

### Reframe 1 — Change the question, not the prompt size

SDG/IMCG needs an **edge**: source-service -> target-service. It does **not** need
`http://medical-data-service:8000/v1/cases`. So ask a **closed-set** question:

> "Which of these N configured services does this call target?"

over the service list already in the configuration. Closed-set classification is:

- trivially validatable — answer must be one of the known services,
- impossible to hallucinate `example.com` into,
- robust to a missing literal URL value.

This single reframe likely moves us off 0%.

### Reframe 2 — Make the model a selector, not a generator

When the literal value *is* needed, do not ask for free-text. Ask the model to
**point at** a variable address from the candidates and state the concat order:

```json
{ "base": "<VariableAddress>", "suffix": "/...", "op": "concat" }
```

Our code does the substitution from the authoritative variable map. The model can
only choose values we already hold — hallucination becomes structurally
impossible.

### Reframe 3 — Retrieve a def-use slice, not a bag

From the residual, parse operand identifiers (`self._mds_url`, `url`). Walk the
chains we already compute:

```
field reference (self._mds_url)
  -> field assignment in __init__ (self._mds_url = base_url)
    -> constructor parameter (base_url)
      -> construction sites of the class (MedicalDataServiceClient(settings.mds))
        -> argument expression (settings.mds)
          -> config/env value (mds = "http://medical-data-service:8000")
```

Assemble **only those links**, in order. The answer is now present, and the
model's job collapses to concatenation — which it does reliably. Bonus:
evidence-grounding validation becomes real (every cited string must appear in the
chain).

### Reframe 4 — Tool-calling retrieval (optional, highest robustness)

Give the model tools: `find_assignments(field)`, `find_construction_sites(class)`,
`lookup_var(name)`, `read_config(key)`. It pulls what it needs iteratively. This
removes the cap/ranking gamble entirely — recall is no longer bounded by a 150
budget. Local Ollama is slow but free, and we only spend round-trips on true
residuals. Trades the thing we have too much of (a flat bag) for the thing we
lack (targeted retrieval). Defer until slicing proves insufficient.

---

## 5. Recommendation

Combine **Reframe 1 + Reframe 3**: closed-set service classification, fed a
def-use slice instead of a variable bag.

- Slice attacks the structural recall failure.
- Closed set attacks the hallucination/validation failure.
- Together they match what the SDG actually consumes.

Add **Reframe 2** to harden the literal-value path. Defer **Reframe 4** until we
have proven slicing alone is not enough.

---

## 6. Concrete code touch-points

| Concern | Current location | Change |
|---|---|---|
| Context unit | `query_builder.rs::build_snippet` (whole-class span) | Replace blob with a cross-file def-use slice from operands |
| Retrieval | `ranking.rs::rank_and_cap` (lexical bag) | Replace with structural slice walk |
| Question kind | `query_builder.rs` (hardcoded `ResolveLookup`) | Route to closed-set service classification |
| Key derivation | `query_builder.rs` (`split('/')` on expression) | Remove; operate on AST operands |
| Answer shape | `response.rs::SageResponse` (free-text `resolved`) | Add selector form (VariableAddress / service id) |
| Validation | `response.rs::validate` (confidence + non-empty evidence) | Membership check + evidence grounding |
| Rewrite | `query_builder.rs::rewrite_target_uri_with_resolution` | Substitute from authoritative map, not string split |

---

## 7. Open question for the next step

Spec first: the **slicer** (which def-use edges to walk, what the chain prompt
looks like) or the **closed-set classification** path (service-set prompt +
membership validator)? Both are needed; order is a sequencing choice.
