## Purpose

Endpoints are reported with the full path a client must call, including every prefix added by router composition (sub-app mounts, router prefixes, router inclusion), in any language and framework that has a route-fact adapter, so REST calls can be matched to them without guessing.

## ADDED Requirements

### Requirement: Router composition prefixes are part of the endpoint URI
The system SHALL report an endpoint's URI as the concatenation, outermost first, of every prefix on the path from the top-level router to the router the endpoint is registered on, followed by the endpoint's own route.

#### Scenario: Sub-application mounted under a prefix
- **WHEN** a sub-application is attached to the top-level application under `/v3` and registers `GET /cases/{case_id}`
- **THEN** the endpoint URI is `/v3/cases/{case_id}`

#### Scenario: Router with its own prefix, included under another prefix
- **WHEN** a router declares its own prefix `/items`, registers `GET /{id}`, and is attached to the application under `/api`
- **THEN** the endpoint URI is `/api/items/{id}`

#### Scenario: Prefix given by a constant
- **WHEN** a router's prefix is the constant `BASE_PATH = "/api/v1/consignservice"` and the router registers `POST /consigns`
- **THEN** the endpoint URI is `/api/v1/consignservice/consigns`

### Requirement: Prefixes follow the router through function calls
The system SHALL resolve the router of an endpoint registered on a function parameter by following every call to that function and the argument passed at each call, across files of the same service.

#### Scenario: Router passed through two helper functions in different files
- **WHEN** the application module mounts `app_v1` at `/v1` and calls `add_routes_v1(app_v1)`, `add_routes_v1(app)` (another file) calls `add_routes_clinical(app)`, and `add_routes_clinical(app)` (a third file) registers `GET /cases/{case_id}` on `app`
- **THEN** the endpoint URI is `/v1/cases/{case_id}`

#### Scenario: Same helper name in two services
- **WHEN** two services each define and call their own `add_routes_v1`, mounted at different prefixes
- **THEN** each service's endpoints get only their own service's prefix

### Requirement: One endpoint per reachable prefix
The system SHALL report an endpoint once for each distinct full prefix through which it is reachable, and SHALL NOT drop any of them.

#### Scenario: Helper reused by two mounted sub-applications
- **WHEN** the same helper registering `GET /alive` is called with a sub-application mounted at `/v1` and with one mounted at `/v3`
- **THEN** two endpoints are reported, `/v1/alive` and `/v3/alive`

### Requirement: Unresolved prefixes keep today's URI
The system SHALL keep the endpoint URI unchanged when no route fact applies or a prefix value cannot be resolved to a string, and SHALL terminate on recursive or cyclic call chains.

#### Scenario: Prefix from an unknown expression
- **WHEN** a router's prefix is `settings.base`, which is not a resolvable string constant
- **THEN** the endpoint keeps its declared URI without a prefix

#### Scenario: Recursive helper
- **WHEN** a helper that registers routes calls itself with its own router parameter
- **THEN** extraction finishes and the endpoint gets the prefixes found on the non-recursive paths

### Requirement: Resolution is language and framework agnostic
The system SHALL resolve prefixes from a language-neutral description of router composition, so that supporting a new framework requires only describing its composition idioms, not changing the resolution. This change SHALL provide that description for Python FastAPI / Starlette (`mount`, `include_router`, `APIRouter(prefix=...)`).

#### Scenario: Same composition from neutral facts
- **WHEN** the resolver is given neutral facts "router `sub` attached to `app` under `/v3`" and an endpoint `GET /x` registered on `sub`, with no source language involved
- **THEN** the endpoint URI is `/v3/x`

#### Scenario: FastAPI idioms produce the facts
- **WHEN** a Python file contains `app.mount(path="/v3/fhir", app=app_v3)`, `app.include_router(r, prefix="/api")` and `r = APIRouter(prefix="/items")`
- **THEN** the facts are: `app_v3` attached to `app` under `/v3/fhir`, `r` attached to `app` under `/api`, `r` has own prefix `/items`

### Requirement: Languages without an adapter are unchanged
The system SHALL produce the same endpoints as before this change for languages and frameworks that report no route facts.

#### Scenario: train-ticket Java regression
- **WHEN** the CLI runs on train-ticket (Java) without `--llm` or `--typesafe`
- **THEN** the set of endpoint URIs equals the set produced before this change
