# agent-loop Specification

## Purpose
One model-agnostic loop that every agentic stage uses: the model either calls a tool or answers, the runtime records each tool result as a cited fact, and a log makes a run resumable.

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
Each model call SHALL return exactly one step. A step SHALL be either a tool call naming one offered tool option, or a final answer with the ids of the facts it cites.

#### Scenario: Tool call then answer
- **WHEN** the model calls one offered tool, then answers on the next call
- **THEN** the session makes two model calls, records one fact, and returns the answer

#### Scenario: Answer at once
- **WHEN** the model answers on the first call
- **THEN** the session makes one model call and records no facts

### Requirement: Runtime-numbered facts
The runtime SHALL run every tool call and record its output as a fact. Fact ids SHALL be numbered `f1`, `f2`, ... in fetch order within one session. Fact text SHALL come only from the tool, never from the model.

#### Scenario: Facts numbered in order
- **WHEN** the model calls two tools in one session
- **THEN** the facts are `f1` and `f2`, in call order, each holding its tool name, arguments and output

#### Scenario: Numbering restarts per session
- **WHEN** a second session starts
- **THEN** its first fact is `f1` again

### Requirement: Answers cite only recorded facts
A final answer SHALL cite only fact ids recorded in the same session. An answer citing any other id SHALL end the session with a model error.

#### Scenario: Unknown citation
- **WHEN** the session holds `f1` and the answer cites `f2`
- **THEN** the session ends with a model error, not an answer

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

#### Scenario: Errored point retried
- **WHEN** the previous run ended point P with a model error
- **THEN** the rerun calls the model for P
