## Why

Endpoints lose route prefixes that the framework adds outside the handler's own route string. Python FastAPI `app.mount("/v3", app_v3)` and `APIRouter(prefix=BASE_PATH)` never reach the endpoint URI, so empaia's `/v1/cases/{case_id}` and `/v3/cases/{case_id}` are both extracted as `/cases/{case_id}`. The matcher cannot tell them apart and keeps both (`suffix` tag); Jev then guesses between identical options and drops the correct endpoint in 34 of 278 ambiguous empaia calls (`jev_triage_runs/matcher-v1/results.md`, Task 8). Spec item #1 in `~/general-notes/research/voyantclair/AI-SAR/change-for-better-precision.md`.

The mechanism is the same in every framework: a route is registered on a router object, and router objects are attached to other routers under a prefix (FastAPI `mount` / `include_router`, Flask blueprints, Django `include`, Go `StripPrefix` / chi `Route`, Express `app.use`). Spring already resolves its class-level prefix inside the Java extractor. A per-framework resolver would be rewritten for each; one shared resolver over neutral facts is written once.

## What Changes

- New language-neutral route-fact IR in `models`: a router gets a prefix of its own, or is attached to a parent router under a prefix.
- Every extractor can report the function that encloses an endpoint registration (`Endpoint.enclosing_function`), so a route registered on a function parameter can be traced to the router the caller passed in.
- One shared resolver in the extraction pipeline (pass 2) composes the full prefix of each endpoint's router across functions and files, using route facts, existing call statements, callables, assignments and the import graph. It is language agnostic.
- Per-language adapters turn framework idioms into route facts. This change ships one adapter: Python FastAPI / Starlette (`mount`, `include_router`, `APIRouter(prefix=)`). Flask, Django, Go and others are later adapters with no resolver change.
- An endpoint reachable through several prefixes becomes one endpoint per prefix. An endpoint whose prefix cannot be resolved keeps its current URI.
- Java output unchanged (Spring prefixes stay in the Java extractor; no Java adapter needed).

## Capabilities

### New Capabilities
- `endpoint-route-prefixes`: endpoint URIs include every prefix added by router composition, resolved across functions and files, for any language with a route-fact adapter.

### Modified Capabilities
<!-- none: no existing specs in openspec/specs/ -->

## Impact

- `models`: new `RouteFact` type; `Endpoint.enclosing_function: Option<String>` (serde default, skipped when `None`).
- `python-extractor`: decorator strategies set `enclosing_function`; new FastAPI route-fact adapter.
- `java-extractor`, `go-extractor`: set `enclosing_function: None` (Go adapter later).
- `extractor-runtime/src/pipeline/pass2/`: per-language adapter dispatch + new shared resolver, run before endpoint post-processing.
- SDG output: empaia endpoint URIs gain `/v1`, `/v3`, `/private/v1`, ... ; polyglot consign / travel-plan gain `/api/v1/...`.
- Expected effect: far fewer `suffix` matches and ambiguous calls on empaia, so far fewer Jev calls (296 now, 161k input tokens).
- Measurement on empaia, train-ticket-polyglot (prefix users) and train-ticket Java (regression only).
