# Sage — LLM Arbiter for Unresolved REST Call Sites

## Why Sage Exists

Static extraction in the VOYANTCLAIR pipeline resolves REST call-site URIs through
symbolic evaluation, constant propagation, and import-graph traversal (see
`cross_file_constant_resolution.md` and `captured_scopes_closure_evaluation.md`).
Even after all those passes, a residual set of call sites remains unresolvable:

- URIs constructed from environment variables injected at runtime (e.g. Spring
  `@Value`, Python `os.getenv`).
- URIs assembled through fluent builder chains (`UriComponentsBuilder.fromHttpUrl(...)`).
- URIs retrieved from map/registry lookups keyed by a runtime value.
- Framework route patterns with path variables that require base-path context
  from class-level annotations.
- Reflective dispatch where the target class or URL is determined at runtime.
- Ambiguous call expressions where the HTTP method cannot be statically determined.

`sage` is a fallback arbiter. It is invoked only after static passes have already
resolved everything they can. It never replaces the static engine — it fills the
gap for cases the static engine cannot handle.

## Crate Layout

```
sage/
  src/
    lib.rs          -- module re-exports
    code.rs         -- CodeSnippet, Symbol, SymbolKind
    facts.rs        -- FactBundle (context bundle sent to LLM)
    messages.rs     -- Message (free-text annotations)
    connector.rs    -- SageClient, SageQuery, SageResponse, SageError, QueryKind
  tests/
    mod.rs          -- integration test (requires live Ollama, marked #[ignore])
```

## Data Structures

### `FactBundle`

`FactBundle` packages all available static context for a single unresolved call
site. It is the only input to the LLM besides the query question.

| Field | Type | Purpose |
|-------|------|---------|
| `sites` | `Vec<CodeSnippet>` | The raw source fragment(s) containing the call site |
| `frameworks` | `Vec<Framework>` | Detected frameworks (Spring, FastAPI, Unknown) |
| `local_scope` | `Vec<Symbol>` | Variables visible in the enclosing function |
| `imported_scope` | `Vec<Symbol>` | Variables pulled in via imports |
| `class_or_module_attrs` | `Vec<Symbol>` | Class fields or module-level attributes |
| `constants` | `Vec<ConstantValue>` | Project constants from `pass2` |
| `others` | `Vec<Message>` | Free-text annotations (e.g. injected `@Value` notes) |

### `CodeSnippet`

Pairs a source fragment with a `Language` tag (`Java` or `Python`). The language
tag is rendered into the LLM prompt so the model applies the right syntactic
conventions.

### `Symbol`

A named binding with an optional resolved value, an optional type, and a `SymbolKind`:

| Variant | Meaning |
|---------|---------|
| `Named` | Local variable in the enclosing scope |
| `Imported { target_file }` | Imported from another file |
| `Attr { class }` | Field of a class (or attribute of a module) |

### `SageQuery`

Pairs a `FactBundle` with a `QueryKind` — the specific question to answer.

### `SageResponse`

The validated answer returned on success:

| Field | Type | Meaning |
|-------|------|---------|
| `resolved` | `Option<String>` | Concrete URI or value; `None` if the model returns null |
| `confidence` | `f32` | Normalised score in [0.0, 1.0] |
| `evidence` | `Vec<String>` | Symbols, constants, or lines the model cited |
| `reasoning` | `Option<String>` | Optional explanation from the model |

## Query Routing

`QueryKind` maps unresolved-call categories to targeted prompt questions:

| Variant | Trigger scenario | Prompt focus |
|---------|-----------------|--------------|
| `ResolveEnvVar { var_name }` | URI sourced from an env var or `@Value` field | What is the value of `var_name`? |
| `ResolveBuilder { chain }` | URI built via a fluent builder chain | What URL does `chain` produce? |
| `ResolveLookup { lookup_key }` | URI retrieved from a map/registry | What does the lookup for `lookup_key` return? |
| `ResolveFrameworkRoute { route_pattern }` | Framework route pattern, needs base path | What is the full URL for `route_pattern`? |
| `ResolveReflective { target }` | Reflective dispatch to unknown class/URL | What does `target` resolve to? |
| `ClassifyHttpCall { call_expr }` | HTTP method is ambiguous | What method and URL does `call_expr` represent? |

## Prompt Architecture

Each `SageClient::query` call builds a three-message prompt:

```
[system]  JSON contract  ->  SageClient::build_system_message()
[user]    FactBundle     ->  SageClient::build_facts_message()
[user]    Question       ->  SageClient::build_question_message()
```

### System message

Instructs the model to respond with exactly one JSON object and nothing else:

```json
{
  "resolved": "<string or null>",
  "confidence": 0.0,
  "evidence": ["<citation>"],
  "reasoning": "<optional string>"
}
```

The contract is enforced by the response validator described below — the system
message sets the expectation; the validator enforces it.

### Facts message (pseudo-DSL rendering of `FactBundle`)

`build_facts_message` renders the bundle as a structured text block:

```
FRAMEWORKS: Spring
SYMBOLS:
  restTemplate (local) -> ? : RestTemplate
  BASE_URL (attr of UserServiceClient) -> ? : String
CONSTANTS:
  BASE_URL = http://user-service:8080 (from src/main/resources/application.properties)
OTHER:
  BASE_URL is injected via @Value("${user.service.base-url}")
SITES:
`Language: Java
Code Snippet: restTemplate.getForObject(BASE_URL + "/api/users", String.class)`
```

Each symbol renders as `name (kind) -> value : type`, where `?` is used when the
value or type is absent.

### Question message

A single focused question derived from `QueryKind`. Every variant ends with:
"Return null if you cannot determine it with confidence >= 0.7." This anchors the
model's own confidence assessment to the same threshold used by the validator.

## Response Validation

After the raw JSON is parsed, two hard checks gate the `Ok` path:

1. **Confidence threshold** — `confidence < 0.7` -> `SageError::LowConfidence`.
2. **Evidence requirement** — `evidence.is_empty()` -> `SageError::MissingEvidence`.

Both thresholds are configurable at `SageClient::new` time via the
`confidence_threshold` parameter. The 0.7 default matches the threshold stated in
every question prompt.

## Ollama Integration

`SageClient` uses `async-openai` pointed at an Ollama-compatible OpenAI-compatible
endpoint. Ollama exposes this at `http://localhost:11434/v1`. A dummy API key
`"ollama"` is required by the `async-openai` config builder but is not validated
by Ollama.

Recommended model: `qwen2.5-coder:7b`. Any code-focused model served by Ollama
that follows the OpenAI chat-completion API is compatible.

```rust
let client = SageClient::new("http://localhost:11434/v1", "qwen2.5-coder:7b", 0.7);
```

## Error Handling

```
SageError::Network(OpenAIError)   -- transport or server error
SageError::Parse(serde_json::Error) -- model returned non-JSON
SageError::LowConfidence { confidence } -- confidence below threshold
SageError::MissingEvidence        -- model cited no evidence
```

All variants implement `std::error::Error` via `thiserror`. Callers should treat
`LowConfidence` and `MissingEvidence` as "no answer available" rather than hard
failures — the call site remains unresolved and can be skipped or flagged for
manual review.

## Data Flow

```
extractor-runtime pass3
  -> unresolved RestCall (env var / builder / etc.)
       |
       v
  build FactBundle
    local_scope + imported_scope + class_or_module_attrs + constants + others
       |
       v
  choose QueryKind based on call-site shape
       |
       v
  SageClient::query(SageQuery { bundle, kind })
    |-- build_system_message()       -> [system] JSON contract
    |-- build_facts_message(bundle)  -> [user]   pseudo-DSL context
    |-- build_question_message(kind) -> [user]   focused question
    |
    v
  Ollama (qwen2.5-coder:7b or similar)
    |
    v
  parse LlmJson
    |-- confidence < 0.7  -> SageError::LowConfidence
    |-- evidence empty    -> SageError::MissingEvidence
    |
    v
  SageResponse { resolved, confidence, evidence, reasoning }
       |
       v
  extractor-runtime: annotate RestCall with resolved URI (if Some)
```

## Walking Skeleton Status

As of the initial implementation, only `ResolveEnvVar` is exercised end-to-end in
the integration test (`sage/tests/mod.rs`). The test is marked `#[ignore]` because
it requires a live Ollama instance. All other `QueryKind` variants have prompt
implementations and compiling call paths, but no hand-labelled evaluation data yet.

Running the ignored test manually:

```bash
# Start Ollama and pull the model first
ollama pull qwen2.5-coder:7b

cargo test -p sage -- --ignored
```

## Known Limitations

- The model may hallucinate a plausible-looking URI that does not correspond to
  any real deployment address. Evidence citations are the primary guard, but they
  are not machine-verified against source positions.
- `confidence` is self-reported by the LLM. There is no external calibration.
  The 0.7 threshold is a heuristic starting point.
- Only `ResolveEnvVar` has been evaluated against real projects. Other variants
  should be treated as experimental until labelled benchmarks are established.
- `sage` has no streaming support. Large `FactBundle`s that produce long prompts
  will block until the model finishes generating its full response.
- The prompt format is plain text, not structured JSON. If the Ollama model
  ignores the JSON contract and wraps the output in markdown code fences, the
  `serde_json::from_str` call will fail with `SageError::Parse`. Strip markdown
  fences in a pre-processing step if this occurs with a specific model.

## See Also

- `docs/cross_file_constant_resolution.md` — static constant propagation that runs before sage
- `docs/captured_scopes_closure_evaluation.md` — closure-scope resolution that runs before sage
- `docs/endpoint_restcall_matching.md` — how resolved URIs are matched to endpoints in the SDG
