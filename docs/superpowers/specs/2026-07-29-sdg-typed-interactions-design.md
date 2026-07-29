# SDG Typed Interactions (A + B + C) — Implementation Design

Date: 2026-07-29
Status: Approved — ready for an implementation plan.
Supersedes: `2026-07-21-sdg-precision-typed-edges-design.md` (the *what*; still the
authority on the problem statement, the RTS priority inversion, and the
source-grounded false-positive inventory). This document is the *how*, and it
changes two things from the earlier spec: the naming (`EdgeKind` ->
`InteractionKind`) and the classifier's input (a signal struct rather than the
domain types). Research framing is logged in
`~/general-notes/research/voyantclair/insights.md`, entry 2026-07-29.

## Problem recap

The reconstructed SDG is the substrate for change-impact analysis and regression
test selection. A missing edge is **unsafe**; a spurious edge is merely
**costly**. So precision may never be bought by deleting an edge.

The audited false positives are mostly not resolution errors — they are real
observations belonging to a *different architectural view*: calls originating in
test code, health/liveness probes, and reflexive (self) calls misattributed to a
like-configured peer. The fix is to classify each interaction and scope the
oracle to the business view, retaining every edge for RTS.

Baselines to be corrected (business-edge precision/recall vs hand-verified
oracles):

- train-ticket Java, no-LLM: 0.98 / 0.95 (87 TP, 2 FP, 5 FN) vs a 92-edge oracle.
  Both FPs are test-origin.
- empaia, LLM + constants + scrape: 0.74 / 0.88 (14 TP, 5 FP, 2 FN) vs a 16-edge
  oracle. 2 FPs are A/B/C; 3 are category-D misresolution (out of scope here).
- Micrograal on the same tree: 0.68 / 0.59.

## Naming

The unit of classification is the **individual request**, not the edge: one
`Connection` can carry requests of different kinds. The edge kind is a rollup
over its requests. `EdgeKind` would therefore be wrong at the level where the
decision is actually made, and "call" is wrong for the message-queue work
landing in parallel. The type is `InteractionKind`; the classifier input is
`InteractionSignals`.

## Core design

### The enum — ordering is the specification

```rust
#[derive(
    Debug, Serialize, Deserialize, ToSchema,
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default,
)]
pub enum InteractionKind {
    #[default]
    Business,    // a real cross-service business dependency
    TestOrigin,  // the call site lives in test code
    Reflexive,   // self-call (localhost / own configured host); source == target
    HealthInfra, // liveness / health probe
}
```

Declaration order carries two rules at once, so neither is written twice:

1. **Per-request precedence** when a request matches more than one non-business
   rule: `TestOrigin` > `Reflexive` > `HealthInfra`. A probe defined inside a
   test is first a test artifact; a self-probe is first a self-call.
2. **Connection rollup**: `Business` wins any tie, which is the RTS-safe
   direction — one real business request keeps the edge in the business view.

```rust
let kind = requests.iter().map(|r| r.kind).min().unwrap_or_default();
```

This also closes a gap in the 2026-07-21 spec, which said a connection takes
"the single non-business category present" and was undefined for a connection
holding one `TestOrigin` and one `HealthInfra` request and no business request.
Under `Ord` that case resolves to `TestOrigin`.

`#[default] Business` plus `#[serde(default)]` on the struct fields gives
backward compatibility: a legacy `sdg.json` without `kind` scores exactly as it
does today.

### The classifier — pure functions over a signal struct

New file `synthesizer/src/sdg/interaction_kind.rs`. No traits, no registry, no
new dependencies, no coupling to the builder.

```rust
pub struct InteractionSignals<'a> {
    pub caller_file: &'a str,  // -> A  where the call site lives
    pub target_path: &'a str,  // -> B  resolved path / method / topic
    pub target_host: &'a str,  // -> C  resolved host, "" if none
}

pub fn classify(s: &InteractionSignals, own_urls: &[String]) -> InteractionKind {
    if is_test_path(Language::from_path(s.caller_file), s.caller_file) {
        return InteractionKind::TestOrigin;
    }
    if is_reflexive(s.target_host, own_urls) {
        return InteractionKind::Reflexive;
    }
    if is_health_path(s.target_path) {
        return InteractionKind::HealthInfra;
    }
    InteractionKind::Business
}

pub fn is_reflexive(host: &str, own_urls: &[String]) -> bool;  // builder needs it early
pub fn host_of(uri: &str) -> &str;
fn is_test_path(lang: Language, path: &str) -> bool;
fn is_health_path(path: &str) -> bool;
```

`own_urls` is the caller's configured URL list verbatim
(`ServiceDescription.urls`, e.g. `http://mds:8000`), **not** a pre-extracted host
list — `is_reflexive` applies `host_of` to each entry itself. Passing the URLs
unmodified keeps the one place that knows how to parse a host inside this file.

`is_reflexive` is called twice per restcall: once in the builder to steer the
matcher, once inside `classify` to tag the result. It is a pure string
comparison, so the duplicate call is not worth caching or threading through as a
parameter.

Early returns in declaration order, so the precedence above is visible in the
source. Everything is a pure function of `&str`, which is what makes the
extension contract below hold.

## Component A — test-origin, the only language-varying rule

`is_test_path` matches on `models::ir::language::Language`:

- **Java** — path contains `/src/test/`, or the file name ends `Test.java`,
  `Tests.java`, or `IT.java`.
- **Python** — a path segment equal to `test` or `tests`, or the file name is
  `conftest.py`, `test_*.py`, or `*_test.py`.
- **Unknown** — the shared path-segment rule only (`/test/`, `/tests/`). This
  gives partial, safe coverage for a language whose arm has not been added yet.

Effect: the two `preserve* -> notification` FPs become `TestOrigin`, taking
train-ticket Java to business precision **1.0**. The edges stay in the SDG, so
RTS can still map `testSendEmail` to notification-service.

`Language::from_path` does not exist yet. The extension-to-language mapping
lives at `extractor-runtime/src/pipeline/pass1.rs:22` as `decide_language`, in a
crate the synthesizer does not depend on. Move it to
`models/src/ir/language.rs` as `Language::from_path`, delete `decide_language`,
and update its two callers (`pass1.rs:12`, `pass3/llm_enhance/signals.rs:53`).
Duplicating the extension match in the synthesizer instead would guarantee the
two copies drift when a language is added.

## Component B — health-infra, language-agnostic

`is_health_path` is a flat const list, matched case-insensitively:

- the final path segment is one of `alive`, `health`, `healthz`, `ready`,
  `readiness`, `live`, `liveness`, `ping`; or
- any whole path segment equals `actuator` (Spring).

Both rules match whole segments, never substrings. An earlier draft of this
spec said "the path contains `/actuator`", which also fired on `/actuator-config`
and `/api/v1/actuatorish-metrics` — business paths that merely share a prefix.
That direction of error is the costly one: a business edge typed `HealthInfra`
leaves the business view entirely and reads as a false negative, and this graph
treats a missing edge as unsafe. Corrected 2026-07-29 during implementation
review; the same segment discipline already governs Component A's `test`/`tests`
rule, which is why `latest/` does not match there.

The path fed in is the matched `endpoint.uri`, falling back to the raw
`restcall.target_uri` when that is empty. The last-segment rule works on a full
URL too, so the fallback needs no separate parser.

Effect: empaia `mds -> event` becomes `HealthInfra`. Health edges are **kept**
in the SDG for RTS.

Note carried over from the prior spec: `mds -> event` is also *misrouted* — its
`target_uri` is `http://auth-service/alive` while the edge target is
event-service, because `_get_service_alive(service)` sweeps a peer list over a
dynamic `service["url"]`. Once typed `HealthInfra` it no longer pollutes the
business score regardless of routing. Fixing the target is a follow-up scoped to
the health-infra view alone.

## Component C — reflexive, as a matcher filter flip

`is_reflexive` tests the host against `localhost`, `127.0.0.1`, `0.0.0.0`,
`::1`, `[::1]`, plus `host_of` each URL configured for the **calling** service.
`own_urls` needs no plumbing — `AssignedRestCall.service.urls` is already in
scope at both call sites.

`host_of` strips the scheme at `://`, takes everything before the first `/`,
`?`, or `#`, drops any `user@` prefix, and strips the port (respecting an IPv6
literal in brackets). Returns `""` for a relative target, which is never
reflexive — so relative URIs behave exactly as they do today.

The matcher at `synthesizer/src/sdg/builder.rs:262` currently hard-skips
same-service pairs, which is why a reflexive call cannot resolve to its own
service and instead fuzzy-matches a peer with a similar configured URL. Invert
the filter for reflexive restcalls rather than adding a separate pass:

```rust
// was: if endpoint.service.name == restcall.service.name { continue; }
let want_self = is_reflexive(host_of(&restcall.data.target_uri), &restcall.service.urls);
if (endpoint.service.name == restcall.service.name) != want_self {
    continue;
}
```

Reflexive restcalls then match *within* their own service and reuse the entire
existing levenshtein machinery, so empaia's `mds -> app` (from
`http://localhost:8000/...`) becomes a real `mds -> mds` self-loop carrying a
real endpoint. No `Option<Endpoint>`, no placeholder `Endpoint::default()`, no
duplicated matching logic. Non-reflexive restcalls still never see own-service
endpoints, so no accidental self-loops appear.

## Consequences and assumptions

Stated explicitly because each is a deliberate call, not an oversight:

- **Self-loops are new edges the SDG has never emitted.** Under `(source,
  target)` scoring a self-loop is not in any oracle and would count as a false
  positive. It is tagged `Reflexive`, so business-only scoring excludes it —
  this is precisely why the tagging has to land together with the filter flip,
  never before it.
- **A reflexive call that matches no own-service endpoint yields no edge.**
  Today it yields a cross-service false positive. Accepted: it was never a
  cross-service edge, and RTS already covers intra-service impact through the
  service node. This is the one case where information is lost rather than
  reclassified.
- **`localhost` is assumed to mean self.** True on empaia and train-ticket.
  It would be wrong in a dev-mode compose setup where `localhost` reaches a
  sibling container.
- **A service calling itself through its own public URL is `Reflexive`,** even
  if the call is business logic. Correct for a dependency graph: a self-loop is
  not a cross-service dependency.

## File-by-file changes

| File | Change |
|---|---|
| `synthesizer/src/sdg/interaction_kind.rs` | **new** — enum, signals, `classify`, `is_reflexive`, `is_test_path`, `is_health_path`, `host_of`, tests (~110 lines) |
| `synthesizer/src/sdg/mod.rs` | `pub mod interaction_kind;` |
| `models/src/ir/language.rs` | add `Language::from_path()` |
| `extractor-runtime/src/pipeline/pass1.rs` | delete `decide_language`, call `Language::from_path` |
| `extractor-runtime/src/pipeline/pass3/llm_enhance/signals.rs` | update the import at line 13 |
| `synthesizer/src/sdg/model/mod.rs` | `kind: InteractionKind` on `Request` and `Connection`, `#[serde(default)]` |
| `synthesizer/src/sdg/builder.rs` | filter flip in `create_endpoint_restcall_pairs`; classify per request and roll up in `create_connections` |
| `synthesizer/src/sdg/model/bolt.rs` | `kind` in the two fallback `Request` literals; recompute `Connection.kind` from requests in `TryFrom<BoltMap>` |

### Persistence — what is stored where

- **`sdg.json`** (`cli/src/main.rs:142`, plain serde) carries `kind` on both
  `Request` and `Connection`. This is what the scoring scripts read.
- **Neo4j**: the Cypher at `queries.rs` does `SET r.requests = conn.requests`,
  and `From<Connection> for BoltType` serializes each `Request` as a whole JSON
  string — so `Request.kind` round-trips with no change. `GET_SDG` returns only
  `{source, target, requests}`, so `TryFrom<BoltMap> for Connection` recomputes
  `Connection.kind` from the requests it just parsed. No new Neo4j property, no
  Cypher change, and no denormalized field that can drift.
  - `ponytail:` a `kind` property on the `DEPENDS_ON` relationship would let
    Cypher consumers filter by view server-side. Add it when an RTS query
    actually needs that, not before.
- `From<Request> for BoltType` (`bolt.rs:183`) splits a request into two keys
  and would drop `kind`, but it has **no callers**. Left untouched — deleting
  dead code here buys nothing and risks a conflict with the parallel branch.

## Data flow

```
extraction (unchanged)
  |
  v
SdgBuilderImpl::build
  |
  +-- create_endpoint_restcall_pairs          [C fires here, before matching]
  |     per restcall:
  |       host      = host_of(restcall.data.target_uri)
  |       want_self = is_reflexive(host, restcall.service.urls)
  |       -> if want_self: consider ONLY own-service endpoints
  |          else:         consider ONLY cross-service endpoints (today's rule)
  |       -> Vec<(AssignedRestCall, AssignedEndpoint)>
  |
  +-- create_connections                      [A and B fire here, after matching]
        per (restcall, endpoint) pair:
          signals = InteractionSignals {
              caller_file: &restcall.data.file_path,          // -> A
              target_path: if endpoint.data.uri.is_empty() {  // -> B
                               &restcall.data.target_uri      //    fallback
                           } else {
                               &endpoint.data.uri             //    matched path
                           },
              target_host: host_of(&restcall.data.target_uri) // -> C
          }
          Request.kind = classify(&signals, &restcall.service.urls)
                           |
                           +-- is_test_path(Language::from_path(caller_file),
                           |                caller_file)      -> TestOrigin
                           +-- is_reflexive(target_host, own_urls)
                           |                                  -> Reflexive
                           +-- is_health_path(target_path)    -> HealthInfra
                           +-- otherwise                      -> Business

        per connection:
          Connection.kind = requests.iter().map(|r| r.kind).min()
                              // Business wins any tie (RTS-safe)
  |
  v
Sdg -> sdg.json via serde        (Request.kind + Connection.kind both present)
    -> Neo4j                     (requests carry kind; Connection.kind recomputed on read)
  |
  v
scoring: count kind == "Business" only
```

Two things this makes explicit that the prose alone did not. **The three
components fire at two different points** — C must run inside
`create_endpoint_restcall_pairs` because it decides *which* endpoints are
eligible, while A and B run in `create_connections` because B needs the endpoint
that matching produced. And `target_path` prefers the **matched** `endpoint.uri`
over the raw `target_uri`, falling back only when the matched URI is empty; the
last-segment health rule works on a full URL, so the fallback needs no separate
path parser.

## Scoring changes

Outside this repository, in `~/muni/sar-compare/`:

- `sdg_compare.py::load_voyant_fromcli` — skip connections where
  `conn.get('kind', 'Business') != 'Business'`. `ground_sdg.py` and
  `ground_empaia.py` both import this loader, so they inherit the change.
- `vc_only_sdg_compare.py::load_voyant_fromcli` — the same one-line change; this
  file holds its own copy of the loader.
- Report the `TestOrigin` / `Reflexive` / `HealthInfra` counts alongside the
  business metrics, so the excluded edges stay visible rather than vanishing.

No CLI flag. Legacy runs have no `kind`, default to `Business`, and score
identically to today, so the change is safe to apply unconditionally.

## Testing

Unit tests in `synthesizer/src/sdg/interaction_kind.rs`:

1. `test_test_origin_from_java_test_path` — `file_path` under `/src/test/` yields `TestOrigin`.
2. `test_test_origin_from_python_test_file` — `test_*.py` and `conftest.py` yield `TestOrigin`.
3. `test_health_probe_typed` — `/alive` and `/actuator/health` yield `HealthInfra`.
4. `test_reflexive_localhost` — `http://localhost:8000/x` from service S is `Reflexive`.
5. `test_reflexive_own_config_url` — the caller's own configured host is `Reflexive`.
6. `test_precedence_test_beats_health` — a probe in a test file is `TestOrigin`.
7. `test_host_of` — port, no port, relative URI, IPv6 literal, `user@host`.
8. `test_rollup_business_wins` — `[Business, HealthInfra].min() == Business`;
   `[TestOrigin, HealthInfra].min() == TestOrigin`.

Builder-level test:

9. `test_reflexive_matches_own_service_endpoint` — a localhost restcall in a
   service that owns a matching endpoint produces one `S -> S` connection typed
   `Reflexive`, and no cross-service connection.

Integration — re-run and re-score. This is a synthesis-only *code* change, but
the CLI has no synthesis-only entry point: `cli/src/main.rs` extracts and
synthesizes in one command, so a re-run re-extracts. That matters differently
for the two corpora:

- **train-ticket Java, no-LLM** — extraction is deterministic and takes ~2.7s
  (`results/train-ticket-java-nollm/run_metadata.json`). This is a **hard gate**:
  business precision must be exactly **1.0** (89 connections, 2 of them typed
  `TestOrigin`), recall unchanged at 0.95 (87 TP, 5 FN).
- **empaia, LLM + constants + scrape** — a re-run re-invokes the LLM, so the
  residual tail is nondeterministic and the 14 TP / 5 FP baseline will not
  reproduce exactly. The acceptance criteria are therefore stated over the
  *classification effect*, not over reproducing the baseline numbers:
  1. the `mds -> event` FP is typed `HealthInfra`, not `Business`;
  2. the `mds -> app` FP is gone, replaced by a `mds -> mds` edge typed `Reflexive`;
  3. business TP count is still 14;
  4. the surviving business FPs are exactly the three category-D edges.
  If the LLM tail shifts other edges between runs, re-run before concluding
  anything — a changed edge outside those four criteria is LLM variance, not a
  regression in this change.

## Extension contract

The three ways this code is expected to be extended, and what each costs:

- **A new language** (Go, TypeScript) — add a `Language` variant plus its
  extension in `Language::from_path`, and one arm in `is_test_path`
  (`*_test.go`, `__tests__/`, `*.spec.ts`). Two files, no logic changes. Until
  the arm is added, `Language::Unknown` still catches `/test/` and `/tests/`
  path segments.
- **A new protocol** (gRPC, message queues — landing in parallel) — construct an
  `InteractionSignals` from the new element type and call `classify`. gRPC fills
  `target_path` with the method (`/grpc.health.v1.Health/Check` is typed
  `HealthInfra` by the existing rule); MQ fills it with the topic and
  `target_host` with the broker. **No edits to `interaction_kind.rs`**, which is
  the point of taking `&str` signals rather than `&RestCall` and `&Endpoint`.
- **A new category** (e.g. `Gateway`, `Discovery`) — add a variant in the right
  precedence position, add a predicate, add one `if` to `classify`. Adding a
  variant is a semantic change to the rollup ordering, so it is a deliberate
  decision point, not an accident.

## Out of scope

- **Category D** — the three remaining empaia FPs (`marketplace -> app` via
  `vault_client.py`, `annotation -> clinical` via `slide_info_url`, `workbench ->
  app` via `frontends.py`/`data.py`). These are constant/host-to-service
  *misresolution*, not classification, and need per-case resolver work. They get
  their own design once A/B/C is measured, held to the same "never delete a real
  edge" bar.
- No extraction-layer file filtering — it would break the RTS test-to-service
  mapping and force re-extraction of every corpus to re-score.
- No IMCG or Context Map changes.
- No confidence gate that drops ambiguous edges.
- No `kind` property on the Neo4j relationship until a query needs it.
