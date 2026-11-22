## MODIFIED Requirements

### Requirement: Service session
Each Jev initiator SHALL first get one service session, in stage `match-service`, keyed by its point id. The answer set SHALL be every configured service that has at least one receiver of the initiator's protocol, the initiator's own service included. Each of these answers is labelled with the service name. After the services, the answer set SHALL hold two fixed answers:
- `external`: a network call to a system that is not a configured service
- `in-process`: no network call, such as a database layer, a cache, a dict or an SDK object

A configured service named `external` or `in-process` SHALL fail the session with a model error rather than overwrite a fixed answer.

The service question SHALL say that a call through a database layer, cache, dict or SDK object is not a service call. It SHALL also say that the caller's own service fits only an HTTP call to that service's own endpoints. The tools SHALL be the `agentic-tools` options for the point, built with the call's operand bindings. The budget SHALL be 6 tool calls.

The task text SHALL name:
- the caller's service, file and function
- the HTTP method
- the call expression's source text, on one line and cut at 300 characters, when the call site is found
- the raw target and the substituted target
- the operand bindings
- each static candidate, with its point id, service and address, or a line saying there was none

#### Scenario: Answer set
- **WHEN** services `a`, `b` and `c` are configured and only `a` and `b` have HTTP endpoints
- **THEN** the service session offers `a`, `b`, `external` and `in-process`

#### Scenario: Call code in the task
- **WHEN** the call is `clients.collection.delete(collection_id=collection_id)`
- **THEN** the task shows that expression, so the model sees the call goes through a client object, not only its target `collection_id`

#### Scenario: Candidates in the task
- **WHEN** a call has two candidates
- **THEN** the task lists both, with their point ids

#### Scenario: Bindings reach the tools
- **WHEN** the call's operand `self._mds_url` is bound to `settings.mds_url`
- **THEN** the session offers `define:mds_url`

### Requirement: Jev outcomes
Each Jev initiator SHALL end in exactly one outcome:

| Service session | Endpoint session | Outcome |
|---|---|---|
| picks service `S` | picks receiver `R` | link to `R`, confidence of the endpoint answer |
| picks `S` | answers none, runs out of budget, or does not run | link to `S` with no receiver, confidence of the service answer |
| picks `S` | model error | `Unmatched`, reason `ModelError` |
| picks `external` | does not run | `Excluded`, reason `External` |
| picks `in-process` | does not run | `Excluded`, reason `InProcess` |
| answers none | — | `Unmatched`, reason `NoFit` |
| runs out of budget | — | `Unmatched`, reason `BudgetExhausted` |
| model error | — | `Unmatched`, reason `ModelError` |

A link SHALL have `MatchSource::Jev`. A link to the caller's own service SHALL have kind `Reflexive`. Any other link's kind SHALL be computed by the same classification as a static link; with no receiver, the classification uses the substituted target. A model error's note SHALL hold the error message. `tried` SHALL hold the static candidates' point ids. An `Excluded` note SHALL hold the answer's confidence and, when there are any, the static candidates' point ids. A pick of `external` or `in-process` below the answer threshold SHALL count as none, like any other pick.

#### Scenario: Endpoint picked
- **WHEN** the service session picks `medical-data-service` and the endpoint session picks `GET /v3/cases/{id}` with probability 0.8
- **THEN** a `Jev` link to that endpoint with confidence 0.8 sits in the dependency from the caller's service to `medical-data-service`

#### Scenario: Service only
- **WHEN** the service session picks `medical-data-service` and the endpoint session answers none
- **THEN** a `Jev` link with no receiver sits in the dependency to `medical-data-service`

#### Scenario: Own service is reflexive
- **WHEN** the service session picks the caller's own service
- **THEN** the link's kind is `Reflexive`, so score.py leaves it out of the business view

#### Scenario: Cache lookup is in-process
- **WHEN** the service session picks `in-process` for `v3_scope_case_cache.get(scope_id)`
- **THEN** the call is in `excluded` with reason `InProcess`, no endpoint session runs, and no link is written

#### Scenario: Vault call is external
- **WHEN** the service session picks `external` for a POST to `{settings.vault_url}/v1/...`
- **THEN** the call is in `excluded` with reason `External`, and in neither `unmatched` nor the SDG

#### Scenario: No service fits
- **WHEN** the service session answers none
- **THEN** the call is in `unmatched` with reason `NoFit`

#### Scenario: Endpoint model error
- **WHEN** the endpoint session fails with a model error
- **THEN** the call is in `unmatched` with reason `ModelError`, and no link is written

### Requirement: Evidence of both sessions
A Jev link, a Jev `Unmatched` or a Jev `Excluded` SHALL carry as evidence the facts of its service session, then those of its endpoint session, renumbered `f1`, `f2`, ... in that order.

#### Scenario: Facts from two sessions
- **WHEN** the service session recorded two facts and the endpoint session one
- **THEN** the link's evidence is `f1`, `f2` from the service session, then `f3` from the endpoint session

#### Scenario: Excluded keeps its facts
- **WHEN** the service session recorded one fact and picked `in-process`
- **THEN** the excluded point's evidence is that fact as `f1`

### Requirement: Jev metrics
With `--jev`, `metrics.json` SHALL hold a `jev` entry. It SHALL count Jev initiators, sessions, replayed sessions, model calls, input tokens and output tokens. It SHALL also count outcomes: links with a receiver, links with no receiver, unmatched by reason, and excluded by reason. The per-protocol `links` count SHALL include Jev links. Without `--jev`, the entry SHALL be absent.

#### Scenario: Counts add up
- **WHEN** a run sends 6 initiators to Jev, and they give 3 receiver links, 1 service link, 1 `NoFit` and 1 `InProcess`
- **THEN** the `jev` entry counts 6 initiators, 3, 1, `NoFit` 1 and excluded `InProcess` 1
