## Context

See proposal.md (Why). Relevant current state:

- Pass 2 identification (`extractor-runtime/src/pipeline/pass2/identification.rs`) calls one `identify(file)` per language, which fills `file.raw_restcalls` from `call_statements`.
  - Python (`python-extractor/src/extraction/restcalls/identification/method_call.rs`): any call whose last name segment is in `HTTP_METHODS` (`get`, `post`, ...) with a URL-ish first argument. Library-agnostic by name; misses `urllib.request.urlopen`.
  - Java (`java-extractor/src/extraction/restcalls/identification/spring.rs`): requires `invoked_on == "RestTemplate"`.
- Pass 3 (`extractor-runtime/src/pipeline/pass3/`) evaluates URIs of `raw_restcalls` and runs the async DDFA residual resolver (`llm_enhance/ddfa.rs`) with an injectable `ask` closure and a fact catalog (`llm_enhance/facts/`).
- `cli/src/lib.rs:50-68`: `build_project_ir` (sync) then `evaluate` (pass 3).
- Jev cost reference: 296 `Choice` questions = 160,937 input tokens (matcher-v1 Task 8).
- Measured static recall to beat: train-ticket service R 0.967, empaia 0.941 (`jev_triage_runs/matcher-v1/results.md`, `static+fuzzy`).

## Goals / Non-Goals

**Goals:**
- Answer one question with numbers: how much of a knocked-out rule's recall does per-symbol Jev spec inference recover, at what precision and token cost, and is a cached rerun deterministic.
- Keep the inference stage language agnostic: candidate selection and spec application work on `CallStatement` / `RestCall`; only the knockout gates live in extractors.

**Non-Goals:**
- Endpoint / route spec inference (second arm, after `endpoint-route-prefixes`).
- Message-queue primitives (RabbitMQ, Kafka).
- Per-call-site Jev classification (only estimated for cost comparison, not built).
- Making spec inference the default.

## Decisions

### D1. Knockout as a rule-name set passed into identification
`identify(file, knockout: &KnockoutSet)` in each extractor; rule names are constants next to the rule (`java-resttemplate` in `spring.rs`, `python-http-method-names` in `method_call.rs`). CLI validates names against the union of known names before extraction.
- Alternative: env var. Rejected: invisible in `run_metadata.json`; the CLI flag is recorded there.

### D2. Candidate selection (static, cheap, overapproximate)
A call statement is a candidate when all hold:
1. not claimed by an active static rule (it produced no `RestCall`);
2. callee is not a callable defined in the project (`ProjectIR.callable_map`) - library symbols only;
3. some argument is URL-like: string literal with `/` or `://`, or an identifier whose name matches `(?i)url|uri|endpoint|host|path` or whose same-file / imported constant value contains `://`.
Symbol key: `invoked_on` type + method when known (`RestTemplate.exchange`), else the import-qualified callee (`urllib.request.urlopen`), else the raw callee text.

### D3. One DDFA-style question set per symbol
New async stage `extractor-runtime/src/pipeline/spec_inference/` run between pass 2 and pass 3 (in `cli/src/lib.rs`), reusing the injectable `ask` closure shape of `ddfa.rs`.
- State: symbol key, language, import line, receiver type, up to 3 sampled call sites (snippet + argument list), deterministic sample order (file path, span).
- Questions (one request): `is_http` Noul ("Does calling this send an HTTP request to another service?"); `url_arg` Choice over argument positions/keywords seen + `none`; `method` Choice over `GET..PATCH` fixed + "argument i" for each position + `unknown`.
- Facts loop: at most 2 extra facts from a small catalog (`more_call_sites`, `library_docstring` if available in a vendored source, `import_context`). Reuse `ddfa.rs` loop if its fact interface fits; otherwise `Once` mode only (decide in task 3.1, record in results).
- Acceptance: `is_http` yes with p >= `JEV_SPEC_MIN` (default 0.7) and `url_arg` != `none`. Method `unknown` -> spec with method "any" (matcher already treats unknown method as non-match today; record count).

### D4. ClientSpec and cache
```rust
pub struct ClientSpec { symbol: String, is_http: bool, url_arg: Option<ArgRef>, method: MethodSource, p: f64 }
pub enum ArgRef { Position(usize), Keyword(String) }
pub enum MethodSource { Fixed(HttpMethod), Arg(ArgRef), Unknown }
```
Cache: JSON map `symbol -> ClientSpec` (rejections stored with `is_http: false`), path from `--spec-cache`, default `<output_dir>/client_specs.json`. Loaded before asking, written after. Keys sorted for diffable files.

### D5. Applying specs
A generic, language-neutral identification strategy builds `RestCall { origin: Spec, http_method, target_uri: arg value, .. }` for each candidate call whose symbol has an accepted spec, appended to `raw_restcalls` before pass 3, so URI evaluation, constant substitution, matching and Jev match all apply unchanged.

### D6. Experiment matrix (runs, not code)
| arm | flags | corpora |
|---|---|---|
| static | none | TT, polyglot, empaia |
| KO | `--knockout X` | TT (`java-resttemplate`), polyglot + empaia (`python-http-method-names`) |
| KO+spec | `--knockout X --typesafe --jev-specs` | same |
| KO+spec cached | same + same cache, 2nd run | same |
| natural+spec | `--typesafe --jev-specs` | TT, polyglot (urllib voucher) |
Metric: recovery = (R_KO+spec - R_KO) / (R_static - R_KO), service level and endpoint level (TT, polyglot); precision; Jev calls, input tokens, symbols asked/accepted; cached-run set diff; estimated per-call-site cost = candidate call sites x tokens per question.

## Risks / Trade-offs

- [Python method-name knockout removes calls whose library is still name-recognizable, so the experiment is easy] -> report recovery per symbol; the RestTemplate arm is the hard case (one symbol family, many call sites).
- [Candidate filter misses URL arguments built from locals] -> measured as "KO recall not recovered, symbol never asked"; list them.
- [Jev accepts a non-HTTP symbol (e.g. a cache `get`)] -> precision drop is the measured cost; provenance `spec` makes the FP attributable.
- [Leakage: Jev knows RestTemplate from training, so recovery overstates performance on truly unknown libraries] -> stated as a threat to validity in the report; the urllib natural arm and a renamed-symbol variant (alias the import) are the control.
- [Determinism without cache] -> 3 uncached runs on one corpus, report variance.

## Migration Plan

None. Opt-in flags; default runs unchanged. Rollback = revert.

## Open Questions

- Whether to keep the stage after the experiment, and as default or opt-in. Decided from the report; does not change this change's tasks.
