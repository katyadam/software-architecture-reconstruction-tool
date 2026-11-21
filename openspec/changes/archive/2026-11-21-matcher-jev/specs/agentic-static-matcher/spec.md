## MODIFIED Requirements

### Requirement: Standalone run without a model
`agentic-cli` SHALL take a project directory, a configuration file, an optional constants file and an output directory. It SHALL extract the project with the existing extraction passes and no LLM. Without `--jev`, it SHALL NOT start or call any model, local or remote. With `--jev`, it SHALL call only the Jev API, as `agentic-jev-matcher` defines. It SHALL never start a local model.

#### Scenario: Run on empaia
- **WHEN** `agentic-cli` runs on empaia with `local-empaia-config.json` and `empaia-constants.json`, without `--jev`
- **THEN** it writes the outputs under the output directory and makes no network call to a model

#### Scenario: No constants file
- **WHEN** no constants file is given
- **THEN** extraction and matching run with no constants

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

### Requirement: Baseline parity
On empaia, with `local-empaia-config.json` (where medical-data-service also has the url `http://mds`) and `empaia-constants.json`, and without `--jev`, `final/sdg.legacy.json` scored by score.py against the empaia oracle SHALL give P 1.000, R 0.882, F1 0.938. That is the static baseline minus its fuzzy match.

#### Scenario: Empaia score
- **WHEN** score.py scores the run against `empaia-ground-truth/sdg-edges.json`
- **THEN** TP 15, FP 0, FN 2, with FN `job-execution-service -> marketplace-service` and `app-service -> medical-data-service`
