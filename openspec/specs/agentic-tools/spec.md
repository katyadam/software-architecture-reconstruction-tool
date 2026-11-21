# agentic-tools Specification

## Purpose
Read-only tools an agentic stage offers a model for one initiator point: code facts from the extracted project, and the receivers it could reach.

## Requirements

### Requirement: Tools for one initiator
The tool set SHALL be built for one initiator point, as defined in `agentic-matches-model`, of any protocol. It SHALL read only the extracted project, the configuration, the constants and the receiver points. It SHALL NOT call a model or change any input. A receiver point SHALL get no tool set. Tool output SHALL be plain text.

#### Scenario: REST call
- **WHEN** the tool set is built for a REST call initiator in `workbench-service`
- **THEN** it offers code facts, `similar`, `imports`, and one `service:<name>` option per configured service with an HTTP receiver

#### Scenario: Message producer
- **WHEN** the tool set is built for a RabbitMQ producer with destination `Queues.queueName`
- **THEN** it offers `define:queueName`, and `similar` ranks RabbitMQ receivers only

### Requirement: Code facts equal to DDFA
The code fact options SHALL be `define:<name>`, `constant:<name>`, `callers`, `receiver` and `open:<file>#<codeword>`. They SHALL have the same ids, offer order and output as the DDFA residual facts for the same call and bindings. Every lookup SHALL stay in the origin file's language. A lookup that is not about one file SHALL also stay in the origin service. Symbol names SHALL come from a REST call's operand bindings, or from the identifiers in a message edge's raw destination.

#### Scenario: DDFA unchanged
- **WHEN** the DDFA residual facts run on the existing fixtures
- **THEN** their option ids and fact bodies are the same as before the change

#### Scenario: Same facts for an agentic point
- **WHEN** the tool set is built for the fixture call `{self._mds_url}/cases/{case_id}` with its bindings
- **THEN** its code fact options are `define:mds_url`, `constant:mds_url`, `callers`, `receiver`, `open:proj/caller/settings.py#settings`, in that order

### Requirement: Similar receivers
`similar` SHALL return at most 10 receivers. They SHALL have the initiator's protocol, and SHALL NOT be in the initiator's service. For HTTP they SHALL also have the initiator's method. They SHALL be ranked by edit distance between addresses, closest first, with ties broken by service name and point id. Unassigned receivers SHALL be included under the empty service.

#### Scenario: Other service only
- **WHEN** `workbench-service` calls `GET /v3/cases`, and `GET /v3/cases` exists in `workbench-service` and in `medical-data-service`
- **THEN** `similar` lists only the `medical-data-service` receiver

#### Scenario: Method filter
- **WHEN** the call is `GET /v3/cases` and the only other-service receiver is `POST /v3/cases`
- **THEN** `similar` returns `(nothing found)`

### Requirement: Receivers of one service
`service:<name>` SHALL be offered once for each configured service name that has at least one receiver with the initiator's protocol, the initiator's own service included. It SHALL list every receiver of that service with the initiator's protocol, of any HTTP method, sorted by address and point id.

#### Scenario: Own service offered
- **WHEN** the initiator is in `workbench-service`
- **THEN** `service:workbench-service` is offered when `workbench-service` has a receiver with the initiator's protocol

#### Scenario: Service without receivers
- **WHEN** service `auth` has no receiver with the initiator's protocol
- **THEN** `service:auth` is not offered

#### Scenario: Protocol kept
- **WHEN** `service:order-service` is called for a RabbitMQ producer, and `order-service` has HTTP endpoints and one RabbitMQ consumer
- **THEN** only the RabbitMQ consumer is listed

### Requirement: Import neighbours
`imports` SHALL list the files the origin file imports, with each codeword, and the project files in the origin's language that import the origin file. The list SHALL be sorted and shown with project-relative paths.

#### Scenario: Both directions
- **WHEN** `client.py` imports `settings` from `settings.py`, and `api.py` imports from `client.py`
- **THEN** `imports` for a call in `client.py` lists `settings.py` as imported and `api.py` as importing

### Requirement: Receiver rows and bounds
Each receiver row SHALL hold the receiver's point id, service and address. HTTP addresses SHALL show the method and the path. Every tool output SHALL be at most 40 lines. When there are more, the output SHALL end with a `... N more` line. An empty result SHALL be `(nothing found)`.

#### Scenario: Long service
- **WHEN** a service has 55 receivers of the initiator's protocol
- **THEN** `service:<name>` shows 40 rows and `... 15 more`

### Requirement: Options offered once
Tool options SHALL be offered best first: code facts, then `similar`, then `imports`, then the `service:<name>` options by name. An option taken in the session SHALL NOT be offered again. Opening a file SHALL add that file's imports to the `open:` options, as DDFA does.

#### Scenario: Taken option
- **WHEN** `similar` has been taken in the session
- **THEN** it is not offered again

#### Scenario: Opened file widens options
- **WHEN** `open:proj/caller/settings.py#settings` is taken, and `settings.py` imports `config` from `config.py`
- **THEN** an `open:` option for `config.py` is offered

### Requirement: Evaluation keeps the project
Evaluation SHALL borrow the extracted project, so a caller can keep it after evaluation. Evaluation SHALL be able to return each REST call's operand bindings, aligned with the evaluated REST calls. The evaluated elements SHALL be the same as before, and the baseline `cli` output SHALL NOT change apart from element order.

#### Scenario: Baseline unchanged
- **WHEN** baseline `cli` runs on empaia with no LLM, before and after the change
- **THEN** `sdg.json` holds the same elements, ignoring order, since two baseline runs already differ in order

#### Scenario: Agentic output unchanged
- **WHEN** `agentic-cli` runs on empaia
- **THEN** every file under `match/` and `final/` is byte-identical to the run before the change
