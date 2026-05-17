# Sage Variables Ranking

## What and Why

Before this change, every `SageQuery` cloned the project-wide variables map
(built by `build_variable_map` in
`extractor-runtime/src/pipeline/pass3/llm_enhance/variables.rs:11`) into its prompt. On a
medium project that is 1000+ entries — every global constant, file-level
assignment, attribute, and module const across every microservice — sent on
every LLM call. The result was hallucination (lost-in-the-middle picks of
plausible-but-wrong entries that shared `_URL`/`_HOST`/`BASE` suffixes),
cross-microservice contamination, non-deterministic prompts (`HashMap`
iteration order), and wasted tokens. The fix is a per-query
relevance-ranked top-N subset: identifier-aware scoring and deterministic
Vec-based ordering preserved through the entire prompt pipeline. The full
map is still computed once for the run -> only the per-query slice is
pruned.

See also `sage_llm_arbiter.md` for the surrounding fallback design.

## Scoring Rubric

Implemented in `score()` at
`extractor-runtime/src/pipeline/pass3/llm_enhance/ranking.rs:50`. Scores are
additive; higher wins; ties broken by `stable_key`.

```text
+100  normalized name appears in the snippet's identifier set
 +40  name is "similar" to a snippet identifier (case-insensitive contains,
      after stripping leading underscores) — catches `self._mds_url` vs
      `MDS_URL`
 +30  value looks like a URL or hostname (`http(s)://`, `://`, host[:port])
 +15  name hints URL-ish (uppercase contains URL/URI/HOST/ENDPOINT/BASE/PORT)
 +20  variable's microservice equals the snippet's microservice
 +10  variable's file equals the snippet's file
  +5  scope is Global (constants outrank class/function-local assignments)
```

The locality bonuses (`+20` microservice match, `+10` file match) are
**soft tie-breaks, not hard filters**. A strong lexical match in a
different microservice (e.g. `_mds_url -> MDS_URL` in another service) will
still outrank a same-microservice variable whose name barely matches. This
is what enables the empaia
`MedicalDataServiceClient._mds_url` case — a constructor-injected field
whose value lives in a different file and different microservice.

## Identifier Extraction

`extract_identifiers` turns raw tokens from the snippet into a `HashSet<String>`
the scorer probes once per candidate variable. Fuzziness is required because
variable names in `variables_map` rarely share the same casing or punctuation
as the references they appear under in the snippet.

### Pipeline

1. `statix::identifiers::identifiers_in_snippet` returns raw identifier tokens
   via tree-sitter. String-literal content and comments are skipped because
   those leaves are not identifier-kind nodes.
2. `self` and `this` are dropped. They appear in every Python/Java snippet as
   method-receiver keywords, never as variable names. Keeping them would
   falsely boost any variable whose name contained the substring `self` or
   `this` via `name_similar`. Pure noise filter.
3. For each surviving raw token, five normalized forms are inserted into the
   output set:

   | Form | Purpose | Example: `_MDS_URL` |
   |------|---------|---------------------|
   | raw | preserves exact-case match | `_MDS_URL` |
   | lowercase | case-insensitive match | `_mds_url` |
   | stripped lowercase | leading `_` removed -> matches `self._mds_url` against `MDS_URL` | `mds_url` |
   | snake parts (lowercased) | `BASE_URL` -> `{base, url}` | `{mds, url}` |
   | camel parts (lowercased) | `mdsUrlClient` -> `{mds, url, client}` | `{mds, url}` |

### Why Multiple Forms Together

`score()` probes the set two different ways:

- `idents.contains(&var_norm)` where
  `var_norm = name.trim_start_matches('_').to_lowercase()` -> needs the
  **stripped lowercase** form present for the `+100` hit.
- `name_similar(i, var_name)` does case-insensitive substring both directions
  -> needs **split parts** so a short variable name like `URL` can
  substring-match a small piece (`url`) instead of trying to match a long
  compound (`mdsurlclient`).

The redundancy is intentional. It lets both code paths fire on the same input
without per-call recomputation. The set is built once per snippet and queried
once per variable -> fast.

### Worked Example

Snippet:

```python
url = f"{self._mds_url}/v3/foo"
return await self._http_client.get(url)
```

Raw tokens from tree-sitter (after dropping `self`):

```
{_mds_url, url, _http_client, get}
```

Final `idents` set after normalization (union, lowercased):

```
{_mds_url, mds_url, mds, url, _http_client, http_client, http, client, get}
```

Candidate `MDS_URL = "http://medical-data-service:8000"` lives in microservice
`medical-data-service`; the snippet is in microservice `app-service`. Scoring:

| Rule | Hit | Points |
|------|-----|--------|
| stripped-lowercase contains `mds_url` | yes | +100 |
| `name_similar` between `url` and `MDS_URL` | yes | +40 |
| value is URL-shaped | yes | +30 |
| name hints URL (`URL` keyword) | yes | +15 |
| microservice match | no | 0 |
| file match | no | 0 |
| scope is Global | yes | +5 |
| **Total** | | **190** |

Contrast with `BASE_URL` in the caller's own microservice: it scores `+40`
(name_similar via `url`) `+30` (value URL-shaped) `+15` (name hints URL)
`+20` (microservice match) `+5` (Global) = **110**. The cross-microservice
lexical match outranks same-microservice locality -> the whole point of the
soft tie-break design.

### Failure Mode Guarded By Splitting

Without `split_camel`, the snippet `client.mdsUrlFetcher` would yield set
`{mdsurlfetcher, ...}`. A global named `URL` would miss the `name_similar`
check: `url` is not a substring of `mdsurlfetcher` in the direction needed
for the `+40` hit when the variable name is shorter than the compound. With
`split_camel` the set also contains `{url}`, so the lookup hits.

## Determinism

The output of `rank_and_cap` is a `Vec<(VariableAddress, String)>` sorted
highest-score-first with `stable_key` as the tie-breaker. That `Vec` is
carried verbatim through the pipeline:

```text
rank_and_cap (Vec)
  -> SageQuery.variables_map: Vec<(VariableAddress, String)>
       -> SageClient::query passes .as_slice()
            -> build_variables_message iterates in given order, no re-sort
```

`stable_key(addr)` returns `(microservice, file, scope_str,
variable_name)`. Identical inputs always produce identical prompts, which
makes runs reproducible and is friendly to upstream prompt caches. Verified
by `determinism_with_shuffled_input_ordering` and
`stable_key_decides_when_scores_truly_tie` in the `ranking_tests` module.

## Budget Knob

`SageClient` holds a `variables_budget: usize` field
(`sage/src/resolver/client.rs:26`); the getter
`SageClient::variables_budget()` (`:52`) is read once per query inside
`build_query_for_restcall`.

Default wiring (CLI):

```rust
// cli/src/main.rs:109
SageClient::new(&args.llm_url, &args.llm_model, 0.7, 150)
```

The `150` is the budget. Raise it to widen the recall window at the cost of
tokens-per-query and risk of lost-in-the-middle dilution; lower it for
tighter contexts on small projects. There is currently no CLI flag — change
the constructor argument and rebuild.

## When Ranking Can Miss

Ranking is purely lexical and value-shape based. It will miss a relevant
variable if **both** of these hold:

1. The variable's name does not appear (even normalized / stripped /
   sub-tokenized) in the snippet's identifier set, **and**
2. The variable's name does not match the URL-ish keyword list
   (`URL|URI|HOST|ENDPOINT|BASE|PORT`) and its value is not URL-shaped.

In that case the candidate scores 0 (plus at most `+35` from
locality + Global), and on a large project it may fall outside the top
`budget` slice. Two responses:

- **Raise the budget** in `cli/src/main.rs` for unusually large projects
  where the relevant variable is being capped out below position 150.
- **Add the missing identifier shape as a heuristic** if a recurring class
  of misses appears — e.g. extend `name_hints_url` with a new keyword, or
  extend `extract_identifiers` to harvest more tokens from the snippet (for
  instance, class names referenced by `isinstance` checks).

## Related Code Pointers

| Symbol | Location |
|--------|----------|
| `rank_and_cap` | `extractor-runtime/src/pipeline/pass3/llm_enhance/ranking.rs:20` |
| `score` | `extractor-runtime/src/pipeline/pass3/llm_enhance/ranking.rs:50` |
| `extract_identifiers` | `extractor-runtime/src/pipeline/pass3/llm_enhance/ranking.rs:96` |
| `identifiers_in_snippet` | `statix/src/identifiers.rs:22` |
| `looks_url_or_host` | `extractor-runtime/src/pipeline/pass3/llm_enhance/ranking.rs:163` |
| `name_hints_url` | `extractor-runtime/src/pipeline/pass3/llm_enhance/ranking.rs:175` |
| `stable_key` | `extractor-runtime/src/pipeline/pass3/llm_enhance/ranking.rs:183` |
| `build_variable_map` | `extractor-runtime/src/pipeline/pass3/llm_enhance/variables.rs:11` |
| `build_variables_message` | `sage/src/resolver/prompt.rs:70` |
| `SageQuery` | `sage/src/resolver/query.rs:20` |
| `SageClient::variables_budget` | `sage/src/resolver/client.rs:52` |
| Default budget wiring | `cli/src/main.rs:109` |

Unit tests covering the above live under
`extractor-runtime/src/pipeline/pass3/llm_enhance/ranking.rs` in the
`#[cfg(test)] mod ranking_tests` block (line 198), and prompt-ordering tests
live in `sage/src/resolver/prompt.rs` in `#[cfg(test)] mod tests` (line 111).
