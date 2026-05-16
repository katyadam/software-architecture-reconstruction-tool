# LLM Usage Approaches

Catalog of every way the LLM is (or could be) plugged into VOYANTCLAIR's
extraction pipeline. Each approach is a self-contained design point. The first
entry is the **currently implemented** approach on branch `llm-sage`. Subsequent
entries are alternatives to be prototyped, measured, and compared in a later
paper.

Cross-references: `sage_llm_arbiter.md` (design), `llm_resolution_performance_and_precision.md` (perf notes).

---

## Approach 1 — Per-Call Single-Shot Arbiter (IMPLEMENTED)

Status: implemented on branch `llm-sage`.
Entry point: `extractor-runtime/src/pipeline/pass3/llm_enhance.rs`.
Crate: `sage`.

### One-line summary

For every REST call left unresolved after static passes, send one chat-completion
request to a local Ollama model with a fixed three-part prompt and a global
variables map; rewrite the call's `target_uri` if the response clears the
confidence and evidence gates.

### Trigger

A `RestCall` reaches `evaluate_restcalls_with_llm` and
`is_restcall_evaluated_enough(rc) == EvalState::NeedsLLM`. The LLM never
overrides a statically resolved call.

### CLI opt-in

LLM evaluation is off by default. The `cli` crate exposes three flags
(`cli/src/main.rs`):

| Flag          | Default                       | Effect                                 |
| ------------- | ----------------------------- | -------------------------------------- |
| `--llm`       | `false`                       | Enables `evaluate_with_llm` pass3 path |
| `--llm-url`   | `http://localhost:11434/v1`   | Ollama OpenAI-compat base URL          |
| `--llm-model` | `qwen2.5-coder:7b`            | Model identifier passed to Ollama      |

With `--llm` absent, the pipeline runs `pass3::evaluate` (static only). With
`--llm` set, a `SageClient` is constructed and threaded into
`pass3::evaluate_with_llm`. Confidence threshold is hard-coded to `0.7` at
client construction.

### Inputs collected per call

`FactBundle` (the structured context attached to each `SageQuery`) currently
holds four fields:

- `sites: Vec<CodeSnippet>` — the raw source bytes spanning `rc.source_span`,
  plus a `Language` tag derived from the file extension.
- `frameworks: Vec<Framework>` — currently empty in `build_query_for_restcall`.
- `scraped_variables: HashMap<String, String>` — currently empty in the
  wiring; the substantive variable data flows through `SageQuery.variables_map`
  instead (see below).
- `others: Vec<Message>` — currently empty.

Separately, `SageQuery.variables_map: HashMap<VariableAddress, String>` carries
every known global / class / attribute / module-constant binding in the
**entire project**. It is built once by `build_variable_map` and cloned into
every query. It is **not** part of `FactBundle` — it is rendered as its own
chat message (see "Prompt structure" below).

#### FactBundle pruning history (relevant to the paper)

Earlier versions of `FactBundle` carried a richer, more structured set of
fields per call site:

```text
sites, frameworks,
local_scope:           Vec<Symbol>,
imported_scope:        Vec<Symbol>,
class_or_module_attrs: Vec<Symbol>,
constants:             Vec<ConstantValue>,
others
```

`Symbol` discriminated `Named` / `Imported { target_file }` /
`Attr { class }`, and `ConstantValue` retained the constant's `source_file`.
The facts message rendered a dedicated `SYMBOLS:` block, and constants were
printed as `name = value (from source_file)`.

That structure was **pruned** in commit `92a8970` ("Prune FactBundle and wire
llm evaluation to CLI via new argument"):

- `local_scope`, `imported_scope`, `class_or_module_attrs`, `constants`
  removed from `FactBundle`.
- `scraped_variables: HashMap<String, String>` added in their place.
- `SYMBOLS:` block dropped from `build_facts_message`; the `fmt_symbols`
  helper and its tests are now commented out in `sage/src/resolver/prompt.rs`.
- Constants no longer carry source-file provenance into the prompt — they are
  flat `key = value` lines.

Consequences for the prompt the model sees:

- The model can no longer distinguish a local from an imported or class-attr
  binding when reasoning about a symbol.
- The model loses the file-of-origin signal that previously let it justify a
  constant by citing the resource file it was defined in.
- The substantive symbol/constant content is still delivered, but flattened
  into the `VARIABLES:` block of `(microservice|file|scope|name) = value`
  lines, carried by `SageQuery.variables_map` rather than `FactBundle`.

For comparative evaluation, treat the pre-pruning structure as a separate
prompt variant. See Approach 2 (slice-scoped facts) for the planned re-add of
structure under a tighter token budget.

### Query kind

Always `QueryKind::ResolveLookup { lookup_key }`, where `lookup_key` is the
substring of `rc.target_uri` before the first `/`. The richer query taxonomy
(`ResolveEnvVar`, `ResolveBuilder`, `ResolveFrameworkRoute`,
`ResolveReflective`, `ClassifyHttpCall`) exists in `sage` but is not wired up.

### Prompt structure

Four chat messages, plain text, no JSON-schema enforcement:

1. `system` — JSON contract: must return one object with
   `resolved | confidence | evidence | reasoning`, no markdown, no prose.
   The confidence scale is now anchored at three points
   (`1.0 = certain`, `0.5 = possible`, `0 = nothing found`) after the pruning
   commit also revised the system message.
2. `user` — facts block: `FRAMEWORKS`, `CONSTANTS`, `OTHER`, `SITES`. The
   `SYMBOLS:` section was dropped along with the `FactBundle` pruning; its
   rendering code remains commented out in `prompt.rs` for reference.
   `CONSTANTS:` now renders flat `key = value` lines from
   `bundle.scraped_variables` (no `from <source_file>` suffix anymore).
3. `user` — `VARIABLES:` block, one line per `(microservice|file|scope|name) = value`,
   sourced from `SageQuery.variables_map`. This is where the bulk of the
   substantive context lives after the prune.
4. `user` — the focused question for the chosen `QueryKind`, always ending with
   "Return null if you cannot determine it with confidence >= 0.7."

### Model and transport

- Ollama via OpenAI-compatible `/v1/chat/completions`.
- Rust client: `async-openai` with a dummy `"ollama"` API key.
- Default model: `qwen2.5-coder:7b`.
- No streaming. No tool use. No function calling. No JSON-mode / format param.

### Response handling

`SageClient::query`:

1. Take the first choice's text content, trim, `serde_json::from_str` into
   `LlmJson`.
2. Reject if `confidence < threshold` -> `SageError::LowConfidence`.
3. Reject if `evidence.is_empty()` -> `SageError::MissingEvidence`.
4. Return `SageResponse { resolved, confidence, evidence, reasoning }`.

Default `confidence_threshold = 0.7`.

### Effect on the pipeline

If `resp.resolved` is `Some`, `apply_query_outcomes` rewrites
`restcalls[i].target_uri` by splicing the model's answer in front of the
original URI's path suffix:

```
original = "USER_SVC/api/users"
resolved = "http://user-service:8080"
new      = "http://user-service:8080/api/users"
```

No provenance is currently tagged on the rewritten call; the `RestCall` carries
no `source = LLM` marker yet.

### Concurrency

`stream::iter(...).buffer_unordered(MAX_CONCURRENT_LLM_QUERIES)` with
`MAX_CONCURRENT_LLM_QUERIES = 4`. Each query is independent — no shared cache,
no batching, no retries.

### Known cost characteristics

- One model round-trip per unresolved call.
- The full project `variables_map` is serialised into every prompt. Token cost
  grows linearly with project size, not with the call's locality.
- No prompt cache, no slice cache, no response cache.

### What this approach establishes for the paper

- Baseline precision and recall on hand-labelled unresolved-call corpus.
- Baseline wall-clock latency per call and per project.
- Baseline token usage.
- Reference point against which every later approach is measured.

### Known limitations to call out

- Single `QueryKind` (`ResolveLookup`) used for all residuals.
- Global variable dump rather than slice-scoped facts.
- `FactBundle` after pruning carries almost no structured signal — `sites`
  plus three near-always-empty fields. All real context goes through the
  flat `VARIABLES:` chat message.
- Symbol kind (`Named` / `Imported` / `Attr`) and constant source-file
  provenance are no longer visible to the model.
- Self-reported confidence; no external calibration.
- No verification that `evidence` strings exist in the source.
- No retries, no fallback model, no temperature control surfaced.
- No provenance tag on rewritten URIs.

---

## Approach 2 — Slice-Scoped Facts via Backward Slicer (PLANNED)

Status: not implemented. Design sketched in
`memory/project_llm_arbiter.md` and `sage_llm_arbiter.md`.

Replace the global `variables_map` dump with a `BackwardSlicer` that does a
bounded BFS from the call site through symbols, attrs, and imports. Three fact
tiers: Tier 1 full chain path, Tier 2 compact siblings, Tier 3 module digests.
Cache slices keyed by `(call_site_id, fact_graph_hash, slicer_version)`.

Expected paper contribution: token-cost reduction and precision delta vs.
Approach 1 with the same model.

---

## Approach 3 — Query-Kind Routing (PLANNED)

Status: query kinds exist; routing logic does not.

Pick the `QueryKind` based on the residual call-site shape:

- env vars -> `ResolveEnvVar` (after env-scraper has run first)
- builders -> `ResolveBuilder`
- map lookups -> `ResolveLookup`
- framework annotations -> `ResolveFrameworkRoute`
- reflective dispatch -> `ResolveReflective`
- ambiguous HTTP verb -> `ClassifyHttpCall`

Expected contribution: per-kind precision breakdown, evidence that targeted
questions beat a single generic prompt at constant token cost.

---

## Approach 4 — Interleaved Static + Arbiter Fixpoint (PLANNED)

Status: not implemented.

Run static passes, then arbiter, then re-run static with newly resolved values
fed back in. Repeat until no new resolutions, capped at 5 iterations. Lets the
LLM unblock chains where one resolved env var enables further static
propagation.

Expected contribution: marginal recall gain per iteration; diminishing-returns
curve.

---

## Approach 5 — Ollama JSON-Mode Schema Enforcement (PLANNED)

Status: not implemented.

Switch from plain-text JSON contract in the system prompt to Ollama's `format`
parameter with a strict JSON schema for `LlmJson`. Eliminates the
markdown-fence parse failure mode and lets the validator focus on semantic
checks.

Expected contribution: parse-failure rate before / after.

---

## Approach 6 — Evidence-Grounded Verification (PLANNED)

Status: not implemented.

After the model returns, machine-verify each `evidence` string against the
project's symbol / constant tables. Reject answers where evidence does not
exist in the source. Treat unverifiable evidence the same as missing evidence.

Expected contribution: precision lift at the cost of recall; quantify the
trade-off.

---

## Approach 7 — Multi-Model Voting / Cascade (PLANNED)

Status: not implemented.

Two designs to compare:

- **Voting** — query N models, take majority on `resolved`.
- **Cascade** — try a fast small model first; escalate to a larger model only
  on `LowConfidence` or `MissingEvidence`.

Expected contribution: latency vs. precision Pareto curve across model sizes.

---

## Approach 8 — Tool-Calling / Agentic Resolution (PLANNED)

Status: not implemented.

Expose the project's symbol table, constant table, and file reader as tools the
model can call. Replace one-shot prompt with an agent loop that requests facts
on demand instead of receiving a pre-built `FactBundle`.

Expected contribution: comparison of upfront-context-dump vs. on-demand-fetch
in both precision and total token usage.

---

## Approach 9 — Prompt / Response Caching (PLANNED)

Status: not implemented.

Cache responses keyed by the canonicalised prompt hash so re-runs over the same
codebase do not re-query. Orthogonal to all other approaches.

Expected contribution: incremental-build cost; cache-hit rates over realistic
edit sessions.

---

## Approach 10 — Fine-Tuned / Domain-Adapted Model (PLANNED)

Status: not implemented.

Fine-tune a small open model on hand-labelled `(FactBundle, resolved URI)`
pairs harvested from prior runs. Compare against the base `qwen2.5-coder:7b`
under the same prompt.

Expected contribution: whether task-specific tuning beats prompt engineering
at fixed model size.

---

## Measurement Plan (applies to every approach)

For each approach, record on the same hand-labelled corpus:

- Resolution precision (correct / produced).
- Resolution recall (correct / total unresolved).
- Wall-clock latency per call (median, p95).
- Tokens in / out per call (median, p95).
- Parse-failure rate.
- Low-confidence / missing-evidence rate.
- Number of model round-trips per call.

Baseline = Approach 1. Every later approach reports deltas against it.
