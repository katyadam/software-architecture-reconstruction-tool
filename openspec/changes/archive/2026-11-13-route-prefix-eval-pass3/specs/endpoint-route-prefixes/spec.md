## ADDED Requirements

### Requirement: Prefix values resolve like REST call URLs
The system SHALL resolve each router prefix expression with the same constant sources and precedence used to resolve REST call URLs: constants scanned from the project, constants supplied externally at run time, class attribute defaults, and module-level bindings visible in the file where the prefix is written. A prefix that resolves for a REST call URL in the same file SHALL resolve to the same string for an endpoint.

#### Scenario: Prefix from an external constant
- **WHEN** a router's prefix is `settings.base`, the run is given external constant `settings.base = "/api"`, and the router registers `GET /items`
- **THEN** the endpoint URI is `/api/items`

#### Scenario: Prefix from a chained constant
- **WHEN** a file declares `ROOT = "/api"` and `BASE = ROOT`, and a router with prefix `BASE` registers `GET /items`
- **THEN** the endpoint URI is `/api/items`

#### Scenario: Prefix constant imported from another file
- **WHEN** `config.py` declares `PREFIX = "/v1"`, `app.py` imports `PREFIX` from `config` and mounts a sub-application under `PREFIX`, and the sub-application registers `GET /cases`
- **THEN** the endpoint URI is `/v1/cases`

#### Scenario: Same expression, same value
- **WHEN** a file uses constant `BASE` both as a router prefix and in `url = BASE + "/x"; requests.get(url)`
- **THEN** the endpoint prefix and the REST call URL use the same resolved value of `BASE`

### Requirement: File-local constants take precedence over project-wide constants
The system SHALL resolve a name in a file to that file's own module-level binding or a binding it imports before using a same-named constant from elsewhere in the project. This SHALL apply to router prefixes and REST call URLs alike. Same-named constants in different services SHALL NOT affect each other.

#### Scenario: Same constant name in two services
- **WHEN** service A's router file declares `BASE_PATH = "/api/v1/a"`, service B's declares `BASE_PATH = "/api/v1/b"`, and each registers `POST /items` on a router with prefix `BASE_PATH`
- **THEN** the endpoint URIs are `/api/v1/a/items` in A and `/api/v1/b/items` in B

#### Scenario: REST call URL uses its own file's constant
- **WHEN** two services each declare `BASE = ...` with different values and a file in the second service has `url = BASE + "/x"; requests.get(url)`
- **THEN** the REST call URL uses the second service's value

### Requirement: Ambiguous project constants do not resolve prefixes
The system SHALL NOT resolve a router prefix from a name that the file neither declares nor imports when that name has more than one distinct value across the project. The endpoint SHALL keep its declared URI, independent of file order.

#### Scenario: Unbound name declared twice elsewhere
- **WHEN** a router's prefix is `BASE`, its file does not declare or import `BASE`, and two other files declare `BASE` with different values
- **THEN** the endpoint keeps its declared URI in every file order

#### Scenario: Unbound name with one value
- **WHEN** a Go file in a package uses prefix `API_BASE` and another file of the package declares `const API_BASE = "/api"`
- **THEN** the endpoint URI starts with `/api`

### Requirement: Router fan-out is bounded
The system SHALL bound the prefix chains kept per endpoint to a fixed limit, so that memory stays bounded. Past the limit, the kept chains SHALL be chosen deterministically. This limit is the only exception to reporting one endpoint per reachable prefix. Time is not bounded by this limit.

#### Scenario: Wide router composition
- **WHEN** several levels of routers are each attached four times with distinct non-literal prefixes, exceeding the limit
- **THEN** the endpoint has at most the limit of chains, the same ones in every run

### Requirement: Endpoint output shape is unchanged
The system SHALL NOT add fields to the serialized endpoint output for prefix resolution. Intermediate prefix information SHALL stay internal to extraction.

#### Scenario: Serialized endpoint fields
- **WHEN** an endpoint with a resolved router prefix is written to extraction output
- **THEN** its serialized fields are the same set as an endpoint without a prefix

## MODIFIED Requirements

### Requirement: Unresolved prefixes keep today's URI
The system SHALL keep the endpoint URI unchanged when no route fact applies or a prefix value cannot be resolved to a string from any constant source, and SHALL terminate on recursive or cyclic call chains. When an endpoint is reachable through several prefix chains, only the chains containing an unresolvable prefix SHALL fall back to the declared URI; resolvable chains SHALL still produce prefixed endpoints.

#### Scenario: Prefix from an unknown expression
- **WHEN** a router's prefix is `settings.base`, which neither the project nor any external constant defines
- **THEN** the endpoint keeps its declared URI without a prefix

#### Scenario: One resolvable and one unresolvable chain
- **WHEN** a helper registering `GET /alive` is called with a sub-application mounted at `/v1` and with one mounted at an undefined `settings.base`
- **THEN** two endpoints are reported, `/v1/alive` and `/alive`

#### Scenario: Recursive helper
- **WHEN** a helper that registers routes calls itself with its own router parameter
- **THEN** extraction finishes and the endpoint gets the prefixes found on the non-recursive paths
