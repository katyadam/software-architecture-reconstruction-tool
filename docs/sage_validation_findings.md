# Sage LLM Arbiter — Validation Findings (what's wrong)

First empirical end-to-end validation of the `sage` arbiter on the **llm-sage** branch.

- **Date:** 2026-06-06
- **Target:** `/butler/empaia` (819 Python + 212 Java files)
- **Model:** `qwen2.5-coder:7b` via local Ollama (`http://localhost:11434/v1`)
- **Params:** `confidence_threshold=0.7`, `variables_budget=150`, only `QueryKind::ResolveLookup` is live
- **Method:** instrumented `SageClient::query` with a `SAGE_TRACE` JSONL dump (query -> raw response -> outcome), added `env_logger::init()` to the CLI (neither existed before), ran the CLI with `--llm`.

## Headline result

**271 LLM queries -> 0 correct resolutions -> 0 SDG connections. Precision 0%.**

(`Number of REST calls to evaluate with LLM: 271`, confirmed in the run log.)

| Outcome | Count | % |
|---|---|---|
| Rejected | 230 | 85% |
| — invalid JSON from the model | 144 | **53%** |
| — confidence < 0.7 | 86 | 32% |
| Accepted (passed validation) | 41 | 15% |
| — garbage / hallucinated (example.com, `{...}`, `${...}`, `[key/url]/value`, `localhost`) | 41 | |
| **Correct (a real empaia service URL)** | **0** | **0%** |
| SDG connections produced | 0 | |

Comparison runs (same checkout):
- Baseline, no `--llm`, no `--scrape`: 0 connections.
- `--scrape` only: 0 connections (see Finding 7).
- `--llm` (this run): 0 connections.

---

## Findings, by severity

### 1. (Critical) No structured-output enforcement — 51% of responses are unparseable JSON

`SageClient::query` (`sage/src/resolver/client.rs`) sends a plain chat request and then does
`serde_json::from_str(text.trim())`. Ollama's OpenAI-compatible `format` / JSON-schema /
grammar enforcement is **not** used, so the model freely emits invalid JSON. 144 of 271
responses (53%) failed to parse and were thrown away after paying full inference cost.

Representative failure (the `MedicalDataServiceClient._mds_url` site):

```json
{
  "resolved": "{{settings.mds}}",
  "confidence": 1.0,
  "evidence": ["..."],
  "reasoning": The value is explicitly set using ...   <-- missing opening quote
}
```
-> `JSON parse failure: expected value at line 5 column 16`.

The original design (`project_llm_arbiter` memory / `sage_llm_arbiter.md`) called for using
Ollama's `format` param. It was never wired. This is the single biggest waste of compute.

### 2. (Critical) The model has nothing to resolve with — it hallucinates placeholders

Every one of the 41 "accepted" (confident, parseable, evidence-bearing) answers is unusable.
The model invents example domains or echoes the variable name back:

```
https://default.mds.example.com      http://example.com/api      https://api.example.com
{base_url_value}                     {{config.server.baseUrl}}   ${BASE_URL}
{constant_value_for_base_url}        {base_api}/resource          {app_base_url}
{host}/api/endpoint                  /api    /api/v1              http://example.com
```

This is a **retrieval problem, not a model problem**. The concrete URL is not present in
what we send, so a confident model fills the blank with a plausible-looking fake. Two
contributing prompt issues:
- The prompt does not strongly instruct "return `resolved: null` if the value is not
  derivable from the given facts/variables" — so the model guesses instead of abstaining.
- `confidence` is self-reported and meaningless here: 9 of the garbage answers above are
  `confidence: 1.0`. The 0.7 threshold filters nothing useful.

### 3. (High) `lookup_key` derivation mangles identifiers

`query_builder.rs::build_query_for_restcall` derives the lookup key with
`rc.target_uri.split('/').next()`. Observed keys fed into prompts:

```
ResolveLookup:self._mds_urlurl      <- "_mds_url" + "url" concatenated
ResolveLookup:url"]"                <- stray bracket/quote
ResolveLookup:{base_url}            <- unresolved f-string placeholder used as the key
```

Garbage keys -> garbage questions. The target_uri these come from is itself an unresolved
expression (e.g. `self._mds_url + url`), so splitting on `/` produces nonsense. The query
builder assumes a URL-shaped `target_uri` that residual calls do not have.

### 4. (High) Resolutions are lossy and untagged (no provenance)

`dispatch.rs::apply_query_outcomes` overwrites `RestCall.target_uri` with the model's answer
and discards `confidence`, `evidence`, and `reasoning` entirely. Consequences:
- No way downstream to distinguish a static edge from an LLM-guessed one.
- No audit trail; no post-hoc thresholding.
- **It actively blocked this validation** — we could only get numbers after adding the
  `SAGE_TRACE` instrumentation. The design memory explicitly required provenance tagging;
  it is not implemented.

### 5. (High) The CLI had no logger at all

`cli/src/main.rs` never called any logger init, so every `info!`/`warn!` in the LLM
pipeline (the residual count, per-query failures, skip reasons) went to a no-op backend.
`RUST_LOG` did nothing. The LLM stage shipped with zero observability. (Fixed this session
by adding `env_logger::init()`.)

### 6. (Medium) Only 1 of 6 query kinds is live

`query_builder` hardcodes `QueryKind::ResolveLookup`. `ResolveEnvVar`, `ResolveBuilder`,
`ResolveFrameworkRoute`, `ResolveReflective`, `ClassifyHttpCall` have prompts and tests but
are dead in the production path. There is no routing that classifies a residual and picks
the right kind — so env-var sites, builder chains, etc. are all asked the wrong question
(`ResolveLookup`).

### 7. (Medium) Env-scraping resolves nothing in this checkout

`--scrape` produced 0 connections because `/butler/empaia` ships `sample.env`,
`development.env`, `sample_external.env`, etc. — not `.env`. The scraper does not pick these
up. This means:
- The "realistic" measurement (scrape -> LLM-on-residuals) cannot be run here without first
  fixing env-file handling; scrape+llm currently degenerates to llm-only.
- The LLM is being handed env-var residuals that a working scraper would have resolved
  cheaply and correctly — inflating both its workload and its hallucination surface.

### 8. (Medium) Validation gate is too weak to catch hallucinations

`response.rs::validate` accepts any answer with `confidence >= threshold` and a non-empty
`evidence` array. It does **not** check:
- URL shape (an answer like `{base_url_value}` or `/api` is accepted),
- that the resolved host matches any known service in the configuration,
- that the cited evidence strings actually occur in the provided snippet/variables
  (grounding).

All 41 garbage answers passed this gate. Self-reported confidence is not a usable signal.

### 9. (Low) Cost / latency

The 271 residual queries at 4-concurrent took ~45 minutes wall-clock on this machine, and
much of it is spent before any LLM call: `rank_and_cap` runs per residual over the full
project variable map during `collect_pending_queries` (CPU-bound, single-threaded), then
half the LLM responses are discarded as unparseable. Net useful output: nothing.

### 10. (Low) Stale doc comment

`extractor-runtime/.../llm_enhance/mod.rs` still documents `ranking` as doing "relevance
ranking + secret redaction"; secret redaction was removed.

---

## Measurement gaps discovered (not bugs in sage, but blockers to evaluating it)

- **No usable ground truth.** `~/Downloads/sdg-empaia.json` is a node-only Neo4j export
  (Service nodes + endpoints, zero relationship records) with malformed nested escapes that
  will not even parse. There is no machine-readable empaia service-call graph to score
  against; precision must be judged by reading call sites in source.
- **No baseline reproduction.** Prior output dirs (`empaia-sar-output-scraping`, etc.) show
  10–16 connections, but those cannot be reproduced in this checkout because env-scraping is
  dead here (Finding 7). The provenance of those prior numbers is unclear.

---

## Suggested order of attack (for discussion, not yet actioned)

1. **#1** Wire Ollama JSON-grammar/`format` enforcement — eliminates the 51% parse loss.
2. **#3** Fix `lookup_key` derivation (or stop deriving a key from an unresolved expression).
3. **#2** Make the prompt demand `null` when the value isn't derivable; stop trusting
   self-reported confidence.
4. **#8** Add URL-shape + service-membership + evidence-grounding checks to `validate`.
5. **#4** Tag arbiter edges with provenance (also unblocks ongoing measurement).
6. **Then re-measure.** Before any of #2/#8, audit how many of the 271 residual sites are
   *even resolvable from available facts* — if the answer is "few," the LLM is aimed at
   unwinnable cases and the design (LLM-on-residuals via ResolveLookup) needs rethinking
   rather than tuning.

## Reproduction

```bash
cargo build --release -p cli
SAGE_TRACE=/tmp/sage-trace.jsonl RUST_LOG=info ./target/release/cli \
  -p /butler/empaia -c config/configurations/empaia-config.json \
  -o empaia-val-llm-traced --llm
# then inspect /tmp/sage-trace.jsonl (one JSON object per query)
```
