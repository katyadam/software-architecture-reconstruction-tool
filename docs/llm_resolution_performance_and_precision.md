# LLM Resolution — Performance and Precision Options

Current state of LLM-enhanced REST call resolution (the `sage` arbiter wired into
`pass3::llm_enhance`) shows two problems:

- **Latency** — approximately 81 seconds per REST call resolution.
- **Precision** — many simple call sites fail to resolve correctly.

This document inventories the root causes in the current implementation and the
options available for addressing each.

See also: `sage_llm_arbiter.md` for the overall arbiter design.

---

## Current Implementation — Root Causes

### Serial dispatch

`extractor-runtime/src/pipeline/pass3/llm_enhance.rs` iterates REST calls and
awaits each `sage.query(...)` before starting the next:

```rust
for rc in restcalls.iter_mut() {
    // ...
    match sage.query(query).await { /* ... */ }
}
```

No concurrency. With N unresolved calls and ~81s each, total wall time is
N * 81s.

### Full variable map cloned per query

`variables.clone()` is passed in every `SageQuery`. The map aggregates constants,
assignments, per-file attrs, and per-file module attrs across the entire project.
For non-trivial codebases this produces a very large prompt, dominating prefill
time and inflating context for the model.

### Single hardcoded query kind

Six `QueryKind` variants exist (`ResolveEnvVar`, `ResolveBuilder`,
`ResolveLookup`, `ResolveFrameworkRoute`, `ResolveReflective`,
`ClassifyHttpCall`), but `llm_enhance.rs` always uses
`QueryKind::ResolveLookup` with `lookup_key` set to the first path segment of
`target_uri`. The pattern-detection step that would route to the appropriate
query kind is not yet implemented.

### Empty FactBundle context fields

```rust
let bundle = FactBundle {
    sites: vec![snippet],
    frameworks: vec![],            // <-- empty
    scraped_variables: HashMap::new(),  // <-- empty
    others: vec![],                // <-- empty
};
```

The LLM has no framework hint (Spring vs FastAPI vs Flask materially changes
resolution semantics) and no scraped environment context. All context lives in
the flat `variables_map` dump.

### No resolution cache

Identical `(VariableAddress, expression_template)` pairs result in repeated LLM
calls. The cache described in the original sage design (see
`sage_llm_arbiter.md`) is not yet wired in.

### No backward slicing

The full variable map is sent regardless of which symbols are reachable from the
call site. The planned `BackwardSlicer` from the sage design has not been
implemented.

---

## Options to Improve Latency

| Option | Effort | Expected Impact |
| --- | --- | --- |
| Parallelize call dispatch with `buffer_unordered(N)` or `JoinSet` | Small | 4-8x speedup (bounded by Ollama concurrency) |
| Implement `BackwardSlicer` and shrink the variables payload | Medium | Large per-call prefill reduction |
| Add a resolution cache keyed by `(VariableAddress, expression)` | Small | Eliminates duplicate calls |
| Set Ollama `keep_alive` to avoid model reload between calls | Trivial | Cuts cold-start latency |
| Use a smaller model for triage (`qwen2.5-coder:3b`) and fall back to a larger model on low confidence | Small | Faster median, slower tail |
| Use Ollama `format: "json"` to constrain decoding | Trivial | Faster + eliminates parse-failure retries |
| GPU acceleration for the inference host | Infra | Hardware-bound; can dwarf software changes |

---

## Options to Improve Precision

| Option | Effort | Expected Impact |
| --- | --- | --- |
| Pattern-match the unresolved expression and dispatch to the correct `QueryKind` instead of always using `ResolveLookup` | Small | Largest precision win available without model change |
| Populate `FactBundle.frameworks` from the project IR | Small | Framework-specific reasoning becomes possible |
| Populate `FactBundle.scraped_variables` and `others` from env/properties scrapers | Small | Adds context the LLM currently cannot see |
| Backward slicing — reduces irrelevant context that distracts small models | Medium | Helps both speed and precision |
| Larger / stronger model — `qwen2.5-coder:14b`, `codestral:22b`, or hosted (Claude Haiku/Sonnet) | Varies | Hardware or cost trade-off |
| Few-shot examples in the system prompt, one per `QueryKind` | Small | Anchors output format and reasoning style |
| Self-consistency: re-query low-confidence answers with a tighter slice and accept only matching results | Medium | Reduces hallucination on edge cases |

---

## Recommended Sequence

The two highest-leverage changes — small diffs, isolated to `llm_enhance.rs` and
`prompt.rs` — are:

1. **Parallelize the dispatch loop.** Replace the sequential `for` loop with
   `futures::stream::iter(...).buffer_unordered(N)`. Likely 4-8x speedup with no
   model change.
2. **Route to the correct `QueryKind` and populate `FactBundle.frameworks`.**
   Pattern-match `rc.target_uri` (and optionally the snippet) to detect env-var
   references, builder chains, framework routes, etc., then construct the
   matching `QueryKind`. Pull framework hints from the project IR.

After those, the largest architectural win remains the `BackwardSlicer` — it
reduces prompt size dramatically and helps precision simultaneously, but is
multi-day work.

---

## Snapshot

Snapshot date: 2026-05-14. Numbers (81s per call) reflect the current
configuration on branch `llm-sage`.
