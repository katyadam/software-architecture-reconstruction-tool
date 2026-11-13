# agentic-matches-model Specification

## Purpose
Shared output contract of the agentic SDG stages. Matcher, verifier and rematcher all read and write one `Matches` document, so stages can be saved, rerun and compared.

## Requirements

### Requirement: One document for every stage
Each agentic stage SHALL read and write one `Matches` document with three parts: the SDG, unmatched points and excluded points. The document SHALL survive a JSON round trip unchanged.

#### Scenario: Round trip
- **WHEN** a `Matches` document with an unmatched point is written to JSON and read back
- **THEN** the result equals the original

#### Scenario: Empty run
- **WHEN** a stage finds no points
- **THEN** it writes a `Matches` document with empty SDG, empty unmatched list and empty excluded list

### Requirement: Dependencies keyed by service pair and protocol
An SDG dependency SHALL be identified by source service, target service and protocol. Two services talking over two protocols SHALL give two dependencies. Each dependency SHALL list its links, and its interaction kind SHALL be the rollup of its links' kinds, with `Business` winning any tie.

#### Scenario: Two protocols
- **WHEN** service `orders` calls `payments` over HTTP and publishes to it over Kafka
- **THEN** the SDG holds two dependencies from `orders` to `payments`, one per protocol

### Requirement: Links point from initiator to receiver
A link SHALL hold the initiating point, an optional receiving point, its interaction kind, its match source (`Static`, `Jev` or `SystemTwo`), an optional confidence and its evidence. A link without a receiving point SHALL mean the target service is known and the receiving point is not.

#### Scenario: Service known, endpoint not
- **WHEN** the model picks the target service but no endpoint
- **THEN** the link has no receiver and the dependency still appears in the SDG

### Requirement: Every unmatched point has a reason
An unmatched point SHALL carry one reason from: `AddressUnresolved`, `NoCandidate`, `NoFit`, `Ambiguous`, `BudgetExhausted`, `DroppedByVerifier`, `ModelError`. It SHALL also carry the candidates the model saw, the receivers banned for it, a note and evidence.

#### Scenario: Provider failure
- **WHEN** the model call times out, returns an error status or returns an unparsable answer
- **THEN** the point is unmatched with reason `ModelError`, not `NoFit`

### Requirement: Excluded points are kept for audit
Points deliberately left out of the SDG SHALL be listed as excluded, with a reason, note and evidence. A point talking to a system outside the analysed project SHALL be excluded with reason `External`.

#### Scenario: Third-party API
- **WHEN** an initiator calls a third-party API outside the project
- **THEN** it appears in the excluded list with reason `External`, not in the unmatched list or the SDG

### Requirement: Connection points keep the original extraction
A connection point SHALL hold an id, its service, its side (`Initiator` or `Receiver`), its protocol, a normalized address and the original code element. The address MAY change during resolution. The code element SHALL never change.

#### Scenario: Unresolved address
- **WHEN** a REST call target cannot be normalized
- **THEN** the point's address is `Unresolved` with the raw target, and the original REST call stays attached

### Requirement: Protocol comes from the code element
The protocol of a point SHALL follow its code element: REST calls and endpoints are `Http`; a message edge takes its own protocol, so gRPC maps to `Grpc`, Kafka to `Kafka` and RabbitMQ to `RabbitMq`.

#### Scenario: gRPC message edge
- **WHEN** a point wraps a message edge with protocol gRPC
- **THEN** the point's protocol is `Grpc`, not a channel protocol

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

### Requirement: Evidence cites recorded facts
Evidence SHALL pair one claim (`Address`, `TargetService` or `Receiver`) with one fact. A fact SHALL store its id, tool name, arguments and tool output. Fact text SHALL be written by the runtime, never by the model.

#### Scenario: Saved fact outlives the session
- **WHEN** a later stage reads a link's evidence
- **THEN** it sees the tool output itself, without access to the earlier model session

### Requirement: Interaction kind shared with the baseline
The interaction kind values (`Business`, `TestOrigin`, `Reflexive`, `HealthInfra`) SHALL be one shared type used by both the baseline synthesizer and the agentic stages, so scores stay comparable.

#### Scenario: Same type
- **WHEN** the synthesizer and an agentic stage label a link
- **THEN** both use the same interaction kind type and values

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
