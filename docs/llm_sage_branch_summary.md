# `llm-sage` branch summary

Snapshot for session handoff. Branch is 5 commits ahead of `main`, clean working tree, pushed to `origin/llm-sage`. No PR open.

## Commits (newest -> oldest)

```
f84f472  ranking + cap for variables sent to LLM
0758234  parallel LLM REST-call eval; FactBundle prune; approach doc
92a8970  FactBundle prune; CLI wired to llm eval via new arg
af40578  gitignore tweak
061aec1  updatelog
```

## Scope

76 files, ~3937 insertions / 65 deletions. Net-new: the `sage` crate, the `llm_enhance` pipeline module, source-span plumbing in both extractors, Python transitive imports.

## New crate: `sage`

LLM arbiter for unresolved REST calls. Wraps Ollama via OpenAI-compatible API (`async-openai`).

- `resolver/facts.rs`, `code.rs` -> `FactBundle` + `CodeSnippet` carry minimal context (snippet + language) to the prompt.
- `resolver/query.rs` -> `SageQuery { bundle, kind, variables_map: Vec<(VariableAddress, String)> }`. `QueryKind` enum: `ResolveEnvVar`, `ResolveBuilder`, `ResolveLookup`, `ResolveFrameworkRoute`, `ResolveReflective`, `ClassifyHttpCall`.
- `resolver/prompt.rs` -> system / facts / variables / question message builders. Constructor-injected-field hint included.
- `resolver/client.rs` -> `SageClient` with `confidence_threshold` + `variables_budget` knobs.
- `resolver/response.rs` -> strict JSON parsing + confidence-threshold validation.

## New pipeline stage: `extractor-runtime/.../pass3/llm_enhance/`

Folder module (decomposed from a single 533-line file for readability):

| File | Responsibility |
|---|---|
| `mod.rs` | Public entry: `evaluate_restcalls_with_llm`, `build_variable_map` |
| `dispatch.rs` | Collect `NeedsLLM` REST calls, dispatch concurrently (4 in-flight), apply outcomes |
| `query_builder.rs` | Build `SageQuery` per REST call, rewrite resolved target URI |
| `variables.rs` | Aggregate project-wide variables map; `microservice_for_file` lookup |
| `ranking.rs` | Per-query relevance scoring + cap (`rank_and_cap`, `score`, `extract_identifiers`, `name_similar`, `looks_url_or_host`, `name_hints_url`, `stable_key`) |

## New shared infrastructure

- `statix/src/identifiers.rs` -> tree-sitter identifier walker, per-language `IDENTIFIER_KINDS` + `ts_language()` exposed by `statix::java` and `statix::python`. Replaces regex-based identifier extraction (regex matched tokens inside string literals and comments -> false positives).
- `statix/src/import_graph.rs` + `python/imports.rs` -> Python transitive import resolution (multi-dotted imports). New integration tests in `statix/tests/python/import_graph.rs`.
- `models/src/source_code.rs` -> `SourceSpan` shared type.
- `models/src/ir/language.rs` -> `Language` enum (`Java`, `Python`, `Unknown`).

## Extractor changes

- `java-extractor/src/extraction/calls/source_span.rs` + `python-extractor/.../source_span.rs` -> compute span (file_path, start byte, end byte) for `CallStatement`.
- Spans propagated through `CallStatement` -> `RestCall` so the LLM stage can reconstruct the snippet text on demand instead of re-parsing.
- Extractor tests refactored to count source spans in assertions.

## CLI wiring

- `cli/src/main.rs` -> new `--llm-url` / `--llm-model` args; constructs `SageClient::new(url, model, 0.7, 150)` (budget hardcoded).
- New e2e test: `cli/tests/e2e/scenario_python_transistent_import.rs` + fixtures under `cli/tests/fixtures/trans-import/`.

## Documentation (new under `docs/`)

- `sage_llm_arbiter.md` -> surrounding fallback design (when sage runs, what it cannot do).
- `sage_variables_ranking.md` -> ranking rubric, identifier-extraction heuristic with worked example, determinism contract, budget knob.
- `llm_resolution_performance_and_precision.md`
- `llm_usage_approaches.md` -> early design exploration.
- `UPDATELOG.md` at repo root.

## Determinism contract

Variables map is `Vec<(VariableAddress, String)>` end-to-end (not `HashMap`). `rank_and_cap` sorts by score desc, `stable_key` ascending tie-break -> identical inputs always produce identical prompts. Carried verbatim to `build_variables_message` (no re-sort).

## Ranking technique in one line

Rule-based identifier-aware lexical retrieval with controlled-vocabulary query expansion. Sparse, deterministic, no embeddings.

Components:
- **Identifier splitting** (Samurai-style): `split_snake` + `split_camel` produce sub-tokens.
- **Normalization**: raw + lowercase + leading-underscore-stripped lowercase forms.
- **Synonym list** (`name_hints_url`): `URL|URI|HOST|ENDPOINT|BASE|PORT`.
- **Set-membership + substring** scoring against the snippet's identifier set.

## Risks / loose ends

- `variables_budget` constant at `cli/src/main.rs:109`; no CLI flag yet.
- End-to-end run against empaia (`MedicalDataServiceClient._mds_url`) discussed but not yet performed.
- No PR open from `llm-sage` -> `main`.
- Pre-existing `needless_as_bytes` clippy warnings in `java-extractor` / `python-extractor` are unrelated to this branch.

## Recently discussed but not actioned

- Secrets-redaction filter was removed (self-hosted LLM -> no exfiltration boundary; in-source secrets are not this tool's responsibility).
- Identifier extraction documented in `docs/sage_variables_ranking.md` with worked empaia example.
