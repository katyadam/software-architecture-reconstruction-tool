# agent-loop Specification

## Purpose
One model-agnostic loop that every agentic stage uses: the model either calls a tool or answers, the runtime records each tool result as a fact, and a log makes a run resumable.

## Requirements

### Requirement: One session per subject
A session SHALL answer one question about one subject in one stage. The subject SHALL be one of:
- a connection point, as defined in `agentic-matches-model`: one place in the code where a service talks to another. That is a REST call site, an HTTP endpoint, or a message producer or consumer, found by the extractors and identified by its point id. The matcher and the rematcher run one session per initiating point.
- a link: one initiating point joined to its receiving point or target service, identified by its link id. The verifier runs one session per link.

A session SHALL be keyed by its stage and its subject id.

#### Scenario: Matcher session for a REST call
- **WHEN** the matcher handles the REST call `requests.get(f"{self._mds_url}/cases/{id}")` in `svc-a/client.py`
- **THEN** the session's subject id is that call's point id, and its answer names the receiver that call reaches

#### Scenario: Verifier session for a link
- **WHEN** the verifier judges the link from that call to `GET /cases/{id}` in `svc-b`
- **THEN** the session's subject id is the link id

### Requirement: One step per model call
Each model call SHALL return exactly one step. A step SHALL be either a tool call naming one offered tool option, or a final answer. A final answer SHALL hold the answer value, an optional confidence and an optional note. It SHALL NOT name facts; the session's facts are its evidence.

#### Scenario: Tool call then answer
- **WHEN** the model calls one offered tool, then answers on the next call
- **THEN** the session makes two model calls, records one fact, and returns the answer

#### Scenario: Answer at once
- **WHEN** the model answers on the first call
- **THEN** the session makes one model call and records no facts

#### Scenario: Confidence and note kept
- **WHEN** the model answers with confidence 0.8 and note "base URL is settings.mds_url"
- **THEN** the session outcome holds the answer, confidence 0.8 and that note

### Requirement: Runtime-numbered facts
The runtime SHALL run every tool call and record its output as a fact. Fact ids SHALL be numbered `f1`, `f2`, ... in fetch order within one session. Fact text SHALL come only from the tool, never from the model.

#### Scenario: Facts numbered in order
- **WHEN** the model calls two tools in one session
- **THEN** the facts are `f1` and `f2`, in call order, each holding its tool name, arguments and output

#### Scenario: Numbering restarts per session
- **WHEN** a second session starts
- **THEN** its first fact is `f1` again

### Requirement: Identical tool calls not repeated
A tool option SHALL be identified by its tool and its arguments. The same tool MAY be called many times in one session with different arguments. An option already taken SHALL NOT be offered again in the session. A call naming an option that was not offered SHALL end the session with a model error.

#### Scenario: Same tool, different files
- **WHEN** the model calls `open_file` on `a.py`, then `open_file` on `b.py`
- **THEN** both calls run and the session records `f1` and `f2`

#### Scenario: Taken option not offered again
- **WHEN** the model has called tool option `define:x`
- **THEN** later calls in that session do not offer `define:x`

#### Scenario: Call to an unknown option
- **WHEN** the model names an option that was not offered
- **THEN** the session ends with a model error

### Requirement: Step budget
A session SHALL run at most the configured number of tool calls. Once the budget is used, the model SHALL get one last call with no tool options offered. If it still does not answer, the session SHALL end as budget exhausted. The same SHALL hold when no tool options remain before the budget is used. A budget of zero SHALL mean one call with no tools.

#### Scenario: Budget reached
- **WHEN** the budget is 2 and the model keeps calling tools
- **THEN** the session records 2 facts, makes a third call with no tool options, and returns that answer

#### Scenario: Budget exhausted
- **WHEN** the last tool-free call does not return an answer
- **THEN** the session ends as budget exhausted

#### Scenario: Tools run out
- **WHEN** all offered options are taken before the budget is used and the model calls a tool again
- **THEN** the session ends as budget exhausted, not as a model error

#### Scenario: Budget zero
- **WHEN** the budget is 0
- **THEN** the session makes exactly one call, offers no tools, and records no facts

### Requirement: Provider failures are model errors
A provider failure SHALL end the session with a model error carrying the provider's message. This covers timeouts, error statuses and unparsable replies. A model error SHALL NOT be reported as a model decision.

#### Scenario: Provider fails mid-session
- **WHEN** the second model call fails
- **THEN** the session ends with a model error that holds the failure message

### Requirement: Session usage reported
Every finished session SHALL report its model call count and the summed input and output tokens.

#### Scenario: Tokens summed
- **WHEN** a session makes two calls of 100 and 150 input tokens
- **THEN** it reports 2 calls and 250 input tokens

### Requirement: JSONL session log
Every session that ends with an answer or as budget exhausted SHALL append one JSON line to the log. The line SHALL hold the stage, the subject id, the prompt hash, the facts, the outcome, the call count and the tokens. Sessions ending with a model error SHALL NOT be logged. The line SHALL be flushed before the session returns.

#### Scenario: Answer logged
- **WHEN** a session answers
- **THEN** the log gains one line holding stage, subject id, prompt hash, facts, answer and tokens

#### Scenario: Model error not logged
- **WHEN** a session ends with a model error
- **THEN** the log is unchanged

### Requirement: Resume from the log
A session whose stage, subject id and prompt hash match a logged line SHALL return the logged outcome and facts without calling the model. The prompt hash SHALL change whenever the task text, the model id or the budget changes.

#### Scenario: Rerun after crash
- **WHEN** the log already holds a line for stage `matcher`, point P and the same prompt hash
- **THEN** the session returns that outcome and makes no model call

#### Scenario: Changed prompt reruns
- **WHEN** the task text for point P changes
- **THEN** the prompt hash differs and the session calls the model

#### Scenario: Changed budget reruns
- **WHEN** the verifier judged link L with budget 0 and now runs L with budget 3
- **THEN** the prompt hash differs and the session calls the model

#### Scenario: Changed adapter settings rerun
- **WHEN** a stage changes its answer options, Jev wording, Jev thresholds or the system-two system prompt
- **THEN** the adapter's model id differs, so the prompt hash differs and the session calls the model

#### Scenario: Errored point retried
- **WHEN** the previous run ended point P with a model error
- **THEN** the rerun calls the model for P

### Requirement: Adapter model ids cover their settings
Each adapter's model id SHALL be its model name plus a hash of every adapter setting that changes the prompt: the answer options, and for Jev the wording and thresholds, for system two the system prompt.

#### Scenario: Wording edited
- **WHEN** the Jev answer question changes from "Which service?" to "Which endpoint?"
- **THEN** the Jev adapter's model id changes, and still starts with the pinned Jev version

### Requirement: Closed-set answers
A stage SHALL give each session its answer options: each one a label, a description and a value. It SHALL also give a `none` value, used when no option fits. The model SHALL only pick a label. The session's answer SHALL be the value of the picked option. A reply naming neither an offered tool option nor an answer label SHALL be a model error.

#### Scenario: Label picked
- **WHEN** the answer options are `orders` and `payments` and the model picks `payments`
- **THEN** the answer is the value of the `payments` option

#### Scenario: Label outside the set
- **WHEN** the model replies with `inventory`, which is neither an answer label nor an offered tool option
- **THEN** the session ends with a model error

### Requirement: Jev adapter
The Jev adapter SHALL send one System One request per model call. The request state SHALL hold the task and every fact recorded so far, with its id, tool, arguments and output. Each request SHALL ask:
- `answer`: a choice among the answer labels plus `none`
- when tool options are offered, also `enough` (does the state suffice to answer) and `next` (a choice among the tool option ids plus `stop`)

The adapter SHALL answer when no tool options are offered, when `enough` reaches the enough threshold, when `next` is `stop`, or when `next` is missing. Otherwise it SHALL call the tool option `next` names. An answer SHALL take the picked label when its probability reaches the answer threshold, with that probability as its confidence. Otherwise it SHALL take the `none` value, with the picked label and its probability in the note. The enough threshold SHALL default to 0.7 and the answer threshold to 0.5. The `next` choice SHALL hold at most the first 254 offered tool options, in the order offered, plus `stop`. An answer label `none`, a tool option id `stop`, or a duplicate label or id SHALL fail the step as a model error without calling the API.

#### Scenario: Not enough, fetch
- **WHEN** Jev answers `enough` 0.4 and `next` `open:a.py#settings`
- **THEN** the step is a call to `open:a.py#settings`

#### Scenario: Enough, answer
- **WHEN** Jev answers `enough` 0.9 and `answer` `payments` with probability 0.8
- **THEN** the step is the `payments` value with confidence 0.8

#### Scenario: Unsure answer abstains
- **WHEN** Jev must answer and picks `payments` with probability 0.3
- **THEN** the step is the `none` value with note `below 0.5: payments 0.3`

#### Scenario: Stop
- **WHEN** Jev answers `next` `stop`
- **THEN** the step is a final answer

#### Scenario: No options, answer only
- **WHEN** a call offers no tool options
- **THEN** the request asks only `answer`, and the step is a final answer

#### Scenario: Too many options
- **WHEN** a call offers 300 tool options
- **THEN** `next` offers the first 254 of them plus `stop`

#### Scenario: Reserved or duplicate id
- **WHEN** a stage offers an answer labelled `none`
- **THEN** the step fails as a model error and no request is sent

### Requirement: System-two adapter
The system-two adapter SHALL send one chat request per model call to a configured OpenAI-compatible endpoint and model. The request SHALL carry the task, every fact recorded so far, the offered tool options and the answer options. It SHALL ask for a JSON reply holding one pick and a note. The pick SHALL be restricted by schema to the offered tool option ids, the answer labels and `none`. When no tool options are offered, the pick SHALL be restricted to the answer labels and `none`. A pick of `none` SHALL be a final answer with the `none` value. A pick of any tool id SHALL be returned as a call, so the loop rejects unoffered ids and ends a spent budget as budget exhausted. A pick of an answer label SHALL be a final answer with the reply's note and no confidence. A reply that is not valid JSON of that shape SHALL be a model error.

#### Scenario: Call after budget spent
- **WHEN** no tool options are offered and the reply picks a tool id anyway
- **THEN** the step is a call, and the session ends as budget exhausted

#### Scenario: Pick a tool
- **WHEN** the reply is `{"pick": "open:a.py#settings", "note": "need the base URL"}`
- **THEN** the step is a call to `open:a.py#settings`

#### Scenario: Pick an answer
- **WHEN** the reply picks the answer label `payments` with note "settings.mds_url points to payments"
- **THEN** the step is the `payments` value with that note

#### Scenario: Tool id and label never collide
- **WHEN** a tool option id equals an answer label
- **THEN** the reply still maps unambiguously to either the call or the answer

#### Scenario: Unparsable reply
- **WHEN** the reply content is not JSON
- **THEN** the step fails as a model error

### Requirement: Context overflow fails the call
The system-two adapter SHALL accept an optional context size. When set, a reply whose prompt token count reaches the context size SHALL be a model error. When set, a reply with no prompt token count SHALL also be a model error. When unset, no check SHALL run.

#### Scenario: Silent truncation caught
- **WHEN** the context size is 8192 and the reply reports 8192 prompt tokens
- **THEN** the step fails as a model error naming the overflow

#### Scenario: Within context
- **WHEN** the context size is 8192 and the reply reports 3000 prompt tokens
- **THEN** the step succeeds

#### Scenario: No check configured
- **WHEN** no context size is set
- **THEN** a reply without usage still succeeds, with 0 tokens counted

### Requirement: Adapter usage reported
Each adapter SHALL report the provider's input and output token counts for every call, so session usage sums real tokens.

#### Scenario: Jev usage
- **WHEN** a System One reply reports 392 input and 65 output tokens
- **THEN** that call adds 392 input and 65 output tokens to the session

### Requirement: Credentials from the environment
Each adapter SHALL read its API key from the environment and SHALL NOT write it to the session log or to any error message. A missing Jev key SHALL fail adapter construction before any session runs.

#### Scenario: Missing Jev key
- **WHEN** `TYPESAFE_API_KEY` is unset
- **THEN** building the Jev adapter fails, and no session starts
