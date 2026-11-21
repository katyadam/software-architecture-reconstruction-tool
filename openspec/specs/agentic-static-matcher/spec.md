# agentic-static-matcher Specification

## Purpose
The first agentic stage: turn every extracted element into connection points, match initiators to receivers by static rules alone (HTTP rules so far), write `Matches` and a score.py-compatible SDG, and count per protocol how many calls would need a model.

## Requirements

### Requirement: Standalone run without a model
`agentic-cli` SHALL take a project directory, a configuration file, an optional constants file and an output directory. It SHALL extract the project with the existing extraction passes and no LLM. Without `--jev`, it SHALL NOT start or call any model, local or remote. With `--jev`, it SHALL call only the Jev API, as `agentic-jev-matcher` defines. It SHALL never start a local model.

#### Scenario: Run on empaia
- **WHEN** `agentic-cli` runs on empaia with `local-empaia-config.json` and `empaia-constants.json`, without `--jev`
- **THEN** it writes the outputs under the output directory and makes no network call to a model

#### Scenario: No constants file
- **WHEN** no constants file is given
- **THEN** extraction and matching run with no constants

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

### Requirement: Constants in call targets
Before matching, constant names in a call's target SHALL be replaced by their values, with the same rule as the synthesizer, shared by both: `{NAME}` holes and bare whole-word names, longest name first. The point's address SHALL carry the substituted target. The point's original element SHALL keep the unsubstituted target.

#### Scenario: Hole replaced
- **WHEN** the target is `{MDS_URL}/v3/cases` and the constants map `MDS_URL` to `http://medical-data-service:8000`
- **THEN** the address holds `http://medical-data-service:8000/v3/cases` and the element still holds `{MDS_URL}/v3/cases`

### Requirement: Every static candidate is a link
Each initiator SHALL be matched by the rules for its address kind, against receivers of the same protocol only. For an HTTP address, candidates SHALL be found with the synthesizer's candidate rules: same HTTP method, service scope by host, path segment unification, and dropping dominated and suffix candidates. There SHALL be no fuzzy fallback: a call that unifies with no endpoint has no candidate. Each candidate SHALL carry the rule that found it: `exact`, `template` or `suffix`. RPC and channel addresses have no rules yet: such an initiator SHALL become `Unmatched` with reason `NoCandidate` and a note naming its protocol, and SHALL be counted as no rules, not as no candidate. An initiator with an unresolved address SHALL become `Unmatched` with reason `AddressUnresolved`.

Without `--jev`, every candidate SHALL become one link with `MatchSource::Static` and no confidence. With `--jev`, only a strong match SHALL become a static link, as `agentic-jev-matcher` defines. A static link's evidence SHALL be one fact with id `f1`, tool `static`, the candidate's rule as its argument, and a text naming the receiver's service, its address and the initiator's substituted target. Links SHALL be grouped into one dependency per (source service, target service, protocol). Without `--jev`, an initiator with no candidate SHALL become `Unmatched` with reason `NoCandidate`.

#### Scenario: Two candidates
- **WHEN** a call has candidates `GET /v3/cases/{id}` in `medical-data-service` and `GET /v3/cases/{id}` in `clinical-data-service`, without `--jev`
- **THEN** two links exist, one in each dependency from the caller's service

#### Scenario: No candidate
- **WHEN** a call has no candidate, without `--jev`
- **THEN** it is in `unmatched` with reason `NoCandidate` and in no dependency

#### Scenario: No fuzzy guess
- **WHEN** a call targets `/v3/case` and the only same-method endpoint is `/v3/cases`
- **THEN** the call has no candidate

#### Scenario: Static link evidence
- **WHEN** a call to `http://mds:8000/v3/cases/{id}` gets the template candidate `GET /v3/cases/{case_id}` in `medical-data-service`
- **THEN** the link's evidence is one fact `f1` with tool `static`, argument `template`, and a text naming `medical-data-service`, `GET /v3/cases/{case_id}` and the target

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

### Requirement: Baseline parity
On empaia, with `local-empaia-config.json` (where medical-data-service also has the url `http://mds`) and `empaia-constants.json`, and without `--jev`, `final/sdg.legacy.json` scored by score.py against the empaia oracle SHALL give P 1.000, R 0.882, F1 0.938. That is the static baseline minus its fuzzy match.

#### Scenario: Empaia score
- **WHEN** score.py scores the run against `empaia-ground-truth/sdg-edges.json`
- **THEN** TP 15, FP 0, FN 2, with FN `job-execution-service -> marketplace-service` and `app-service -> medical-data-service`
