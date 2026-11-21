# agentic-jev-matcher Specification

## Purpose
The Jev stage of the agentic matcher. It lets Jev decide every HTTP call that the static rules cannot settle: first the target service, then the endpoint, from closed sets. Each decision keeps the tool facts it was based on as evidence.

## Requirements

### Requirement: Jev only on request
`agentic-cli` SHALL run Jev only when `--jev` is given. With `--jev`, it SHALL read the Jev key from `TYPESAFE_API_KEY`. A missing key SHALL fail the run before extraction, with no output written.

#### Scenario: Missing key
- **WHEN** `--jev` is given and `TYPESAFE_API_KEY` is unset
- **THEN** the run fails before extraction and writes no files

#### Scenario: Flag off
- **WHEN** `--jev` is not given
- **THEN** no Jev session runs and the outputs equal the static matcher's

### Requirement: Strong matches stay static
With `--jev`, an HTTP initiator with exactly one candidate tagged `exact` or `template` SHALL become a static link, as `agentic-static-matcher` defines, with no session. Every other assigned HTTP initiator SHALL go to Jev. That covers no candidate, several candidates, and a single `suffix` candidate. A REST call always has an HTTP address. Initiators of other protocols SHALL NOT go to Jev. Unassigned initiators SHALL stay out of matching. Jev initiators SHALL be handled in point id order.

#### Scenario: One template candidate
- **WHEN** a call has the single template candidate `GET /v3/cases/{id}` in `medical-data-service`
- **THEN** it gets a static link and no session

#### Scenario: Tie goes to Jev
- **WHEN** a call has candidates in `medical-data-service` and `clinical-data-service`
- **THEN** it goes to Jev, and no static link is written for it

#### Scenario: Suffix only goes to Jev
- **WHEN** a call's only candidate is tagged `suffix`
- **THEN** it goes to Jev

#### Scenario: No candidate goes to Jev
- **WHEN** a call has no candidate
- **THEN** it goes to Jev

#### Scenario: Message stays static
- **WHEN** a Kafka producer is an initiator and `--jev` is given
- **THEN** no session runs for it, and it is unmatched as without `--jev`

### Requirement: Service session
Each Jev initiator SHALL first get one service session, in stage `match-service`, keyed by its point id. The answer set SHALL be every configured service that has at least one receiver of the initiator's protocol, the initiator's own service included. Each answer is labelled with the service name. The tools SHALL be the `agentic-tools` options for the point, built with the call's operand bindings. The budget SHALL be 6 tool calls.

The task text SHALL name:
- the caller's service, file and function
- the HTTP method
- the raw target and the substituted target
- the operand bindings
- each static candidate, with its point id, service and address, or a line saying there was none

#### Scenario: Answer set
- **WHEN** services `a`, `b` and `c` are configured and only `a` and `b` have HTTP endpoints
- **THEN** the service session offers `a` and `b`

#### Scenario: Candidates in the task
- **WHEN** a call has two candidates
- **THEN** the task lists both, with their point ids

#### Scenario: Bindings reach the tools
- **WHEN** the call's operand `self._mds_url` is bound to `settings.mds_url`
- **THEN** the session offers `define:mds_url`

### Requirement: Endpoint session
When the service session picks a service, the initiator SHALL get one endpoint session, in stage `match-endpoint`, keyed by its point id. The answer set SHALL be the receivers of the picked service with the initiator's protocol, and with the same method when the initiator's method is known. Each answer is labelled `e1`, `e2`, ... in order of address, then point id. Its description holds the address and handler function, and its value is the receiver's point id. When more than 254 receivers qualify, the 254 closest to the substituted target SHALL be kept. The task SHALL be the service task plus the picked service. The tools and the budget SHALL be as in the service session. When no receiver qualifies, no endpoint session SHALL run.

#### Scenario: Method filter
- **WHEN** the picked service has `GET /x` and `POST /x` and the call is a GET
- **THEN** the endpoint session offers only `GET /x`

#### Scenario: Service without endpoints of the method
- **WHEN** the picked service has no GET endpoint and the call is a GET
- **THEN** no endpoint session runs, and the outcome is a link with no receiver

### Requirement: Jev outcomes
Each Jev initiator SHALL end in exactly one outcome:

| Service session | Endpoint session | Outcome |
|---|---|---|
| picks service `S` | picks receiver `R` | link to `R`, confidence of the endpoint answer |
| picks `S` | answers none, runs out of budget, or does not run | link to `S` with no receiver, confidence of the service answer |
| picks `S` | model error | `Unmatched`, reason `ModelError` |
| answers none | — | `Unmatched`, reason `NoFit` |
| runs out of budget | — | `Unmatched`, reason `BudgetExhausted` |
| model error | — | `Unmatched`, reason `ModelError` |

A link SHALL have `MatchSource::Jev`. A link to the caller's own service SHALL have kind `Reflexive`. Any other link's kind SHALL be computed by the same classification as a static link; with no receiver, the classification uses the substituted target. A model error's note SHALL hold the error message. `tried` SHALL hold the static candidates' point ids.

#### Scenario: Endpoint picked
- **WHEN** the service session picks `medical-data-service` and the endpoint session picks `GET /v3/cases/{id}` with probability 0.8
- **THEN** a `Jev` link to that endpoint with confidence 0.8 sits in the dependency from the caller's service to `medical-data-service`

#### Scenario: Service only
- **WHEN** the service session picks `medical-data-service` and the endpoint session answers none
- **THEN** a `Jev` link with no receiver sits in the dependency to `medical-data-service`

#### Scenario: Own service is reflexive
- **WHEN** the service session picks the caller's own service
- **THEN** the link's kind is `Reflexive`, so score.py leaves it out of the business view

#### Scenario: No service fits
- **WHEN** the service session answers none
- **THEN** the call is in `unmatched` with reason `NoFit`

#### Scenario: Endpoint model error
- **WHEN** the endpoint session fails with a model error
- **THEN** the call is in `unmatched` with reason `ModelError`, and no link is written

### Requirement: Evidence of both sessions
A Jev link or a Jev `Unmatched` SHALL carry as evidence the facts of its service session, then those of its endpoint session, renumbered `f1`, `f2`, ... in that order.

#### Scenario: Facts from two sessions
- **WHEN** the service session recorded two facts and the endpoint session one
- **THEN** the link's evidence is `f1`, `f2` from the service session, then `f3` from the endpoint session

### Requirement: Session log and replay
With `--jev`, sessions SHALL be logged to `match/sessions.jsonl` under the output directory, as `agent-loop` defines. A rerun with the same input, model and prompts SHALL replay the logged sessions without calling Jev, and give identical output files.

#### Scenario: Rerun replays
- **WHEN** a finished `--jev` run is repeated into the same output directory
- **THEN** no Jev call is made, and every output file except the log is byte-identical

### Requirement: Jev metrics
With `--jev`, `metrics.json` SHALL hold a `jev` entry. It SHALL count Jev initiators, sessions, replayed sessions, model calls, input tokens and output tokens. It SHALL also count outcomes: links with a receiver, links with no receiver, and unmatched by reason. The per-protocol `links` count SHALL include Jev links. Without `--jev`, the entry SHALL be absent.

#### Scenario: Counts add up
- **WHEN** a run sends 5 initiators to Jev, and they give 3 receiver links, 1 service link and 1 `NoFit`
- **THEN** the `jev` entry counts 5 initiators, 3, 1 and `NoFit` 1
