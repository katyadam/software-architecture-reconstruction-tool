## ADDED Requirements

### Requirement: Links have a stable id
A link SHALL be identified by its initiator point id plus its receiver point id. A link without a receiver SHALL be identified by its initiator point id plus the target service of its dependency. The id SHALL be the same across runs on the same source.

#### Scenario: Pair link
- **WHEN** a link from initiator `i` reaches receiver `r`
- **THEN** its id is initiator `i` with receiver `r`

#### Scenario: Service-only link
- **WHEN** a link from initiator `i` has no receiver and its dependency targets service `payments`
- **THEN** its id is initiator `i` with service `payments`

#### Scenario: Same initiator, two receivers
- **WHEN** one publisher links to two consumers
- **THEN** the two links get different ids

### Requirement: Verdicts judge one link
A verdict SHALL name one link by its id and carry a decision (`Keep`, `Drop` or `Unverified`), a reason and evidence. Verdicts SHALL survive a JSON round trip unchanged.

#### Scenario: Round trip
- **WHEN** a `Drop` verdict with a reason and evidence is written to JSON and read back
- **THEN** the result equals the original

### Requirement: Points expose their location
A connection point SHALL report its location: the file path of its code element, plus the byte span when the element is a REST call with a non-empty span. Other elements SHALL report the file path with no span.

#### Scenario: REST call with span
- **WHEN** a point wraps a REST call in `a/Client.java` with span 10-20
- **THEN** its location is `a/Client.java` with span 10-20

#### Scenario: Endpoint
- **WHEN** a point wraps an endpoint in `b/Routes.java`
- **THEN** its location is `b/Routes.java` with no span

## MODIFIED Requirements

### Requirement: Stable, readable point ids
A point id SHALL be built from file path, function hash, element kind and raw address, joined by `#`. For an endpoint, the raw address SHALL be the HTTP method, a space, then the URI. A REST call with a non-empty source span SHALL append `#<start>-<end>`. The id SHALL contain no line numbers and SHALL be identical across runs on the same source.

#### Scenario: Two calls in one function
- **WHEN** one function holds two REST calls to the same target at different spans
- **THEN** the two points get different ids

#### Scenario: No span
- **WHEN** a REST call in `a/Client.java`, function hash `h1`, targets `http://orders/api` and has an empty span
- **THEN** its id is `a/Client.java#h1#rest_call#http://orders/api`

#### Scenario: One route, two methods
- **WHEN** one function registers `GET /x` and `POST /x` in `app.py` with function hash `h3`
- **THEN** the ids are `app.py#h3#endpoint#GET /x` and `app.py#h3#endpoint#POST /x`
