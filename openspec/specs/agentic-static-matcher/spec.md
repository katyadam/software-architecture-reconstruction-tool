# agentic-static-matcher Specification

## Purpose
The first agentic stage: match REST calls to endpoints by static rules alone, write `Matches` and a score.py-compatible SDG, and count how many calls would need a model.

## Requirements

### Requirement: Standalone run without a model
`agentic-cli` SHALL take a project directory, a configuration file, an optional constants file and an output directory. It SHALL extract the project with the existing extraction passes and no LLM. It SHALL NOT start or call any model, local or remote.

#### Scenario: Run on empaia
- **WHEN** `agentic-cli` runs on empaia with `local-empaia-config.json` and `empaia-constants.json`
- **THEN** it writes the outputs under the output directory and makes no network call to a model

#### Scenario: No constants file
- **WHEN** no constants file is given
- **THEN** extraction and matching run with no constants

### Requirement: REST points with services
Every evaluated REST call SHALL become an initiator point and every endpoint a receiver point, as defined in `agentic-matches-model`. Each point SHALL get the service whose base directory matches its file path, with the same rule as the synthesizer. That rule SHALL exist once, shared by synthesizer and matcher. A point with no matching service SHALL be logged with its file path and counted in the metrics. An unassigned call SHALL be left out of matching. An unassigned endpoint SHALL stay a candidate under the empty service, as in the synthesizer. Configured services with the same name SHALL be one service in the SDG. Message edges SHALL be ignored.

#### Scenario: Assigned call
- **WHEN** a call lives in `empaia/workbench-service/workbench_service/client.py`
- **THEN** its point's service is `workbench-service`

#### Scenario: Unassigned endpoint
- **WHEN** an endpoint's file lies under no configured base directory
- **THEN** it is a candidate under the empty service, a log line names its file, and the metrics count it

### Requirement: Constants in call targets
Before matching, constant names in a call's target SHALL be replaced by their values, with the same rule as the synthesizer, shared by both: `{NAME}` holes and bare whole-word names, longest name first. The point's address SHALL carry the substituted target. The point's original element SHALL keep the unsubstituted target.

#### Scenario: Hole replaced
- **WHEN** the target is `{MDS_URL}/v3/cases` and the constants map `MDS_URL` to `http://medical-data-service:8000`
- **THEN** the address holds `http://medical-data-service:8000/v3/cases` and the element still holds `{MDS_URL}/v3/cases`

### Requirement: Every static candidate is a link
For each initiator, candidates SHALL be found with the synthesizer's candidate rules: same HTTP method, service scope by host, path segment unification, dominated and suffix candidates dropped, fuzzy fallback. Every candidate SHALL become one link with `MatchSource::Static`, no confidence and no evidence. Links SHALL be grouped into one dependency per (source service, target service, HTTP). An initiator with no candidate SHALL become `Unmatched` with reason `NoCandidate`.

#### Scenario: Two candidates
- **WHEN** a call has candidates `GET /v3/cases/{id}` in `medical-data-service` and `GET /v3/cases/{id}` in `clinical-data-service`
- **THEN** two links exist, one in each dependency from the caller's service

#### Scenario: No candidate
- **WHEN** a call has no candidate
- **THEN** it is in `unmatched` with reason `NoCandidate` and in no dependency

### Requirement: Kinds equal to the synthesizer
Each link's kind SHALL be computed by the same classification as the synthesizer, from the call's file, the matched endpoint's path and the substituted target. The classification SHALL exist once, shared by synthesizer and matcher. A dependency's kind SHALL be the rollup of its links' kinds, `Business` winning any tie.

#### Scenario: Test call
- **WHEN** the call lives in a test file
- **THEN** its link kind is `TestOrigin`

#### Scenario: Rollup
- **WHEN** a dependency has one `Business` link and one `HealthInfra` link
- **THEN** the dependency kind is `Business`

### Requirement: Output files
The run SHALL write, under the output directory:
- `match/sdg.json`: the SDG of `Matches`.
- `match/unmatched.json`: the unmatched points.
- `match/sdg.legacy.json`: the same SDG in the synthesizer's JSON shape (`services`, `connections` with `source_id`, `target_id`, `kind`, `requests` of `endpoint` and `restcall`, and an empty `message_connections`). The `restcall` SHALL be the original element.
- `match/metrics.json`: the counts below.
- `final/sdg.json` and `final/sdg.legacy.json`: copies of the match files, since no later stage ran.
Services and dependencies SHALL be sorted by name, so two runs on the same input give identical files.

#### Scenario: Scoreable output
- **WHEN** score.py reads `final/sdg.legacy.json`
- **THEN** it parses without error

#### Scenario: Deterministic output
- **WHEN** the run is repeated on the same input, in any checkout
- **THEN** every output file is byte-identical

### Requirement: Fan-out metrics
`metrics.json` SHALL count, over initiators: total, no candidate, exactly one candidate, several candidates in one service, and candidates in several services. It SHALL also count receivers, unassigned initiators, unassigned receivers, links and dependencies.

#### Scenario: Tie kinds counted
- **WHEN** one call has two candidates in `medical-data-service` and another has candidates in two services
- **THEN** the same-service tie count and the cross-service tie count each include one of them

### Requirement: Baseline parity
On empaia, with `local-empaia-config.json` and `empaia-constants.json`, `final/sdg.legacy.json` scored by score.py against the empaia oracle SHALL give P 0.941, R 0.941, F1 0.941, the same edges as the static baseline.

#### Scenario: Empaia score
- **WHEN** score.py scores the run against `empaia-ground-truth/sdg-edges.json`
- **THEN** TP 16, FP 1, FN 1, with FP `workbench-service -> annotation-service` and FN `job-execution-service -> marketplace-service`
