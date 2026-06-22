# Sage Ingestion / Precondition Audit

Answers the precondition from [`sage_resolution_redesign.md`](sage_resolution_redesign.md)
§3: *where do the values that resolve residual REST-call targets actually live,
and are they in what we ingest today?* Companion to
[`sage_validation_findings.md`](sage_validation_findings.md).

- **Date:** 2026-06-22 (rev. same day — corrected: compose/`.env` **are** ingested; see §0)
- **Corpus:** `/butler/empaia` (~20 services)
- **Method:** static inventory of `settings.py` URL settings, `.env` variants,
  `docker-compose.yml`, the `env-scraper` crate, the CLI constants path, and
  `config/constants/empaia-constants.json`, cross-checked against
  `config/configurations/empaia-config.json`.

---

## 0. Correction to the first draft

The first draft of this audit claimed docker-compose / `.env` are **not**
ingested and that ~62% of URL values are missing from the corpus. **That was
wrong.** They are ingested:

- `env-scraper` parses `.env*` and `docker-compose*.{yml,yaml}`
  (`SourceKind::DotEnv`, `SourceKind::DockerCompose`).
- Values reach the extractor as `external_constants` two ways
  (`cli/src/main.rs`): the `--scrape` flag (`env_scraper::scrape(project_dir)`)
  and/or a pre-built constants file via `-f` (`config/constants/empaia-constants.json`).
- The constant-scanner microservice does the same via
  `scraper/service.rs` -> persisted to PostgreSQL.

So the blocker is **not** ingestion. The real gap is **key reconciliation** —
see §3. (`pass3/env.rs` builds an *in-source* symbol table only; that is a
separate layer from `external_constants`, which is what carries the scraped
values.)

---

## 1. The canonical chain, traced end-to-end

Residual `self._mds_url + url` (`app-service/.../mds_client.py:148`):

```
mds_client.py:148   self._http_client.get(self._mds_url + url)      <- residual
mds_client.py:15    self._mds_url = mds_url                          (same class -> in snippet)
__init__.py:5       MedicalDataServiceClient(mds_url=mds_url)        (other file: construction site)
__init__.py:1       from ....singletons import mds_url               (other file: import)
singletons.py:15    mds_url = settings.mds_url.rstrip("/")           (other file: module global + transform)
settings.py:8       mds_url: str = "http://medical-data-service:8000" (other file: literal value)
```

The chain crosses **four files**, passes a module singleton and a `.rstrip("/")`
transform, and constructs with a **keyword argument**. The class name
`MedicalDataServiceClient` is a loud structural signal independent of any value.

---

## 2. The values are present — keyed to source identifiers

`config/constants/empaia-constants.json` (25 entries) already holds the resolved
URLs, keyed to the **exact source names**, including the residual operand itself:

```
self._mds_url -> http://medical-data-service:5000
self._cds_url -> http://clinical-data-service:8000
cds_url       -> http://clinical-data-service:8000
es_url        -> http://examination-service:8000
this.sosUrl   -> http://shout-out-service        (Java)
settings.mps_url -> http://marketplace-service-mock:8000
```

These URLs **are** canonical and **do** match the config service names. So for
empaia the resolving value is not missing — when this constants file is supplied,
`self._mds_url` is directly available.

---

## 3. The real gap: scraped keys ≠ source identifiers

The **live** scraper (`env_scraper::scrape`, dotenv/compose parsers) emits **raw
env-var keys** — prefixed, uppercased:

```
MDS_CDS_URL: http://clinical-data-service:8000
WBS_MEDICAL_DATA_SERVICE_URL: http://medical-data-service:5000
AS_MDS_URL:  http://medical-data-service:5000
```

But source code references **unprefixed, lowercased** identifiers
(`settings.cds_url`, `self._mds_url`). Nothing in the scrape path maps
`MDS_CDS_URL` -> `cds_url` -> `self._mds_url`. The Pydantic `env_prefix`
(`MDS_`, `WBS_`, `AS_`, ...) that ties an env var to a setting is not modelled.

`empaia-constants.json` works only because its keys were **hand-normalised** to
source identifiers — it is a curated golden file, not raw scraper output. That
curation is exactly the missing automated step.

Two further narrowing gaps:

1. **`${VAR}` placeholders are dropped** by the compose parser (priority 0). So
   indirected values like `WBS_MARKETPLACE_SERVICE_URL: ${MPS_URL}` and
   `AS_MPS_URL: ${MPS_URL}` are never captured.
2. **Dev-placeholder defaults** in `settings.py` (`http://aaa`, `http://mds`,
   `http://as`) do not match canonical config URLs — a value->service match must
   not trust them over a strong name signal.

---

## 4. Why the names save us regardless

Setting / variable names map cleanly to config service names — directly or via
standard abbreviations:

```
medical_data_service_url -> medical-data-service
mds_url / self._mds_url  -> medical-data-service
cds_url                  -> clinical-data-service
job_execution_service_url-> job-execution-service
js_url                   -> job-service
as_url                   -> annotation-service
hs_url                   -> harpy-service
```

One genuine **name collision**: `es_url` could be *event-service* or
*examination-service*. The ingested value disambiguates
(`-> examination-service`). So name-only classification has a small residual
ambiguity that a (correctly-keyed) value resolves.

---

## 5. The residual-population gate (`is_restcall_evaluated_enough`)

Before any of the above matters, one function decides which restcalls a resolver
ever sees (`pass3/restcalls.rs`). After symbolic evaluation, each `target_uri`
is bucketed:

```rust
empty                                   -> Junk     // discard
starts_with("http")                     -> Enough   // eval produced a URL, skip LLM
split('/').next() contains "url"|"uri"  -> NeedsLLM // sent to the resolver
everything else                         -> Junk     // discard
```

So the resolver population is: **eval failed to produce an http URL** ∩ **the
first `/`-segment of the leftover expression lexically contains `url`/`uri`**.
Everything else is silently dropped as `Junk`.

**This is a lexical-naming filter — the same fragile axis we critique
elsewhere, applied earlier and more bluntly, and it has two failure modes:**

1. **It Junks genuine residuals whose base is not named `*url*`/`*uri*`** —
   `self._base + "/v3/slides"`, `self.host + path`, `settings.cds + suffix`
   (base `cds`, not `cds_url`) are all discarded *before any resolver sees
   them*. This **neutralises the classifier upstream**: classification is
   supposed to be robust to bad variable names via structural signals, but this
   gate kills the bad-named residuals first, so that robustness never gets to
   apply. Worse, it is a **recall hole invisible to precision metrics** — the
   validation's "271 queries -> 0%" is precision on the *survivors*; there is no
   number for how many real residuals were Junked.

2. **`split('/')` assumes path-shaped input** (same bug class as
   `query_builder::lookup_key`). Residuals are expressions with usually no `/`,
   so `next()` returns the whole expression; a trailing `+ url` operand then
   satisfies the filter accidentally, and a base named `self._mds` (no `_url`)
   only passes because of the `+ url`. The discriminator is accidental.

A `RestCall` **already is an HTTP call** (it carries `http_method`; it was
identified as a restcall). Re-testing "does the target contain `url`" is a weak,
redundant proxy. The gate should become **structural**, not lexical:

- `Enough` — eval produced a concrete `http...` URL.
- `NeedsResolution` — eval left a non-empty, non-literal residual, *regardless
  of naming*.
- `Junk` — only genuinely empty / non-HTTP / local-path noise.

That widens the population to all true residuals and lets classification do the
discriminating, where the naming signal belongs — not in a hard upstream cut.

---

## 6. Conclusions for the rework

1. **Classification-first remains the right primary lever.** It needs neither the
   value nor key reconciliation — it maps name + structural context -> service,
   so it is robust to the §3 key-shape gap entirely. Names in empaia are strong.

2. **The value path's blocker is key reconciliation, not ingestion.** Compose /
   `.env` are already scraped; the missing automated step is mapping raw env
   keys (`MDS_CDS_URL`) through the Pydantic `env_prefix` to source identifiers
   (`cds_url` / `self._cds_url`). Plus `${VAR}` placeholder resolution.

3. **The curated `empaia-constants.json` masks the gap** — it pre-bridges keys by
   hand. Do not mistake a green run with that file for a working pipeline; the
   `--scrape` path alone would not produce those keys.

4. **Why does 0% persist when the value is available?** With `self._mds_url`
   present in `external_constants`, the residual should resolve **statically**,
   never reaching sage. That it still fails points at *consumption* — whether
   symbolic evaluation looks up `self.`-prefixed operands against
   `external_constants` at all — not at ingestion. This is the thread to pull
   next, and it needs a pipeline run, not static grep.

5. **The lexical gate (§5) may be the largest silent loss.** No resolver helps a
   residual that is Junked before it arrives. Sizing the Junk bucket is a
   prerequisite to trusting any precision number.

---

## 7. Recommended sequencing (updated)

1. **Pipeline run on empaia with `empaia-constants.json`**, tracing every
   restcall through `is_restcall_evaluated_enough`, and **counting all three
   buckets**:
   - `Enough` — resolved statically (sanity: is `self._mds_url` among them?),
   - `NeedsLLM` — the gated residual set (the "271"),
   - **`Junk` — split into `empty` vs `non-empty-but-failed-lexical-gate`.**
     The second figure is the **silent recall hole**: real eval-failures
     discarded purely for not containing `url`/`uri`. This number decides
     whether fixing the gate outranks building the classifier.
2. **Make the gate structural** (§5): `NeedsResolution` = any non-empty,
   non-literal residual on an HTTP-client call, regardless of naming. Re-run and
   compare the new `NeedsResolution` count against the old `NeedsLLM` count to
   quantify the recovered population.
3. Build the **closed-set service classifier** (Reframe 1) fed signals in
   precedence order: resolved value (if any) > structural symbol (client class /
   import) > variable/setting name. Measure precision on empaia.
4. Add **key reconciliation** to the scrape path (env_prefix mapping + `${VAR}`
   resolution) so the `--scrape` path matches what the curated file does by hand.
5. Re-measure on **train-ticket** (40 near-identical `ts-*` names) — the naming
   stress test that decides whether a slicer is still needed.
