## RENAMED Requirements

- FROM: `### Requirement: REST points with services`
- TO: `### Requirement: Points with services`

## MODIFIED Requirements

### Requirement: Points with services
Every evaluated REST call SHALL become an initiator point and every endpoint a receiver point, as defined in `agentic-matches-model`. Every message producer SHALL become an initiator point and every message consumer a receiver point. Message bindings and queue or topic declarations SHALL NOT become points; they SHALL be counted per protocol as topology. A gRPC message point SHALL get an RPC address of service and method. A message destination SHALL get the same constant substitution as a call target. A Kafka or RabbitMQ message point SHALL get a channel address of its destination name and, when known, its exchange as broker. A message edge with an empty destination SHALL get an unresolved address holding the raw destination.

Each point SHALL get the service whose base directory matches its file path, with the same rule as the synthesizer. That rule SHALL exist once, shared by synthesizer and matcher. A point with no matching service SHALL be logged with its file path and counted in the metrics. An unassigned initiator SHALL be left out of matching. An unassigned receiver SHALL stay a candidate under the empty service, as in the synthesizer. Configured services with the same name SHALL be one service in the SDG.

#### Scenario: Assigned call
- **WHEN** a call lives in `empaia/workbench-service/workbench_service/client.py`
- **THEN** its point's service is `workbench-service`

#### Scenario: Unassigned endpoint
- **WHEN** an endpoint's file lies under no configured base directory
- **THEN** it is a candidate under the empty service, a log line names its file, and the metrics count it

#### Scenario: RabbitMQ producer
- **WHEN** a RabbitMQ producer publishes to destination `Queues.queueName`
- **THEN** it is an initiator point with protocol RabbitMQ and channel address `Queues.queueName`

#### Scenario: Binding is topology
- **WHEN** a RabbitMQ binding edge is extracted
- **THEN** it is not a point and the RabbitMQ topology count includes it

### Requirement: Every static candidate is a link
Each initiator SHALL be matched by the rules for its address kind, against receivers of the same protocol only. For an HTTP address, candidates SHALL be found with the synthesizer's candidate rules: same HTTP method, service scope by host, path segment unification, dominated and suffix candidates dropped, fuzzy fallback. RPC and channel addresses have no rules yet: such an initiator SHALL become `Unmatched` with reason `NoCandidate` and a note naming its protocol, and SHALL be counted as no rules, not as no candidate. An initiator with an unresolved address SHALL become `Unmatched` with reason `AddressUnresolved`.

Every candidate SHALL become one link with `MatchSource::Static`, no confidence and no evidence. Links SHALL be grouped into one dependency per (source service, target service, protocol). An initiator with no candidate SHALL become `Unmatched` with reason `NoCandidate`.

#### Scenario: Two candidates
- **WHEN** a call has candidates `GET /v3/cases/{id}` in `medical-data-service` and `GET /v3/cases/{id}` in `clinical-data-service`
- **THEN** two links exist, one in each dependency from the caller's service

#### Scenario: No candidate
- **WHEN** a call has no candidate
- **THEN** it is in `unmatched` with reason `NoCandidate` and in no dependency

#### Scenario: Message initiator without rules
- **WHEN** a Kafka producer is an initiator
- **THEN** it is in `unmatched` with reason `NoCandidate`, a note naming Kafka, and in no dependency

#### Scenario: Protocols kept apart
- **WHEN** service `a` has an HTTP link and a RabbitMQ link to service `b`
- **THEN** they are in two dependencies, one per protocol

### Requirement: Kinds equal to the synthesizer
Each HTTP link's kind SHALL be computed by the same classification as the synthesizer, from the call's file, the matched endpoint's path and the substituted target. The classification SHALL exist once, shared by synthesizer and matcher. Non-HTTP links do not exist yet; their kind rule comes with the first non-HTTP matching rules. A dependency's kind SHALL be the rollup of its links' kinds, `Business` winning any tie.

#### Scenario: Test call
- **WHEN** the call lives in a test file
- **THEN** its link kind is `TestOrigin`

#### Scenario: Rollup
- **WHEN** a dependency has one `Business` link and one `HealthInfra` link
- **THEN** the dependency kind is `Business`

### Requirement: Output files
The run SHALL write, under the output directory:
- `match/sdg.json`: the SDG of `Matches`, all protocols.
- `match/unmatched.json`: the unmatched points, all protocols.
- `match/sdg.legacy.json`: the HTTP dependencies of the SDG in the synthesizer's JSON shape (`services`, `connections` with `source_id`, `target_id`, `kind`, `requests` of `endpoint` and `restcall`, and an empty `message_connections`). The `restcall` SHALL be the original element.
- `match/metrics.json`: the counts below.
- `final/sdg.json` and `final/sdg.legacy.json`: copies of the match files, since no later stage ran.
Services and dependencies SHALL be sorted by name and protocol, so two runs on the same input give identical files.

#### Scenario: Scoreable output
- **WHEN** score.py reads `final/sdg.legacy.json`
- **THEN** it parses without error

#### Scenario: Deterministic output
- **WHEN** the run is repeated on the same input, in any checkout
- **THEN** every output file is byte-identical

### Requirement: Fan-out metrics
`metrics.json` SHALL count per protocol, over initiators: total, no candidate, no rules for the protocol, unresolved address, exactly one candidate, several candidates in one service, and candidates in several services. Per protocol it SHALL also count receivers, topology elements, unassigned initiators, unassigned receivers and links. It SHALL count dependencies in total. A protocol with no extracted element SHALL be absent.

#### Scenario: Tie kinds counted
- **WHEN** one call has two candidates in `medical-data-service` and another has candidates in two services
- **THEN** the HTTP same-service tie count and the HTTP cross-service tie count each include one of them

#### Scenario: Message points counted
- **WHEN** train-ticket is run with `local-train-ticket-config.json`
- **THEN** the metrics hold a RabbitMQ entry, and each of its initiators counts as no rules or unresolved address
