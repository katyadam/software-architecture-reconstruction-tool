## Context
Built on `agentic-sdg`:
- `agentic-matcher::match_static` builds the points, runs the HTTP candidate rules, links every candidate and groups the links into dependencies. It is sync and needs no model.
- `agent-loop` provides `run_session(key, task, model, tools, budget, log)` and the `JevModel` adapter. A `JevModel` is built per session, with that session's `Answers<A>`.
- `agentic-tools::PointTools::new(ctx, point, bindings)` gives the tools for one initiator. Its `ToolCtx` needs the `ProjectIR`, the config, the constants and all receivers.
- `evaluate_with_bindings` returns the bindings aligned by index with `EvaluatedIR.restcalls`. Nothing keys them by point yet.
- `legacy.rs` already writes a request with no endpoint for a link with no receiver.

## Goals / Non-Goals

**Goals:**
- Jev decides every non-strong HTTP initiator, with evidence.
- The run without `--jev` stays sync, offline and deterministic.
- The Jev path can be tested with a scripted model, without the network.

**Non-Goals:**
- `Prior`, the rematcher and the verifier. `Prior` arrives with the rematcher.
- Triage (is this really a call?) and `Excluded`/`External`.
- Message protocols, both-sides receiver resolution.
- Parallel sessions.
- Changing the baseline `cli`, the synthesizer or DDFA.

## Decisions

### Split the static pass, then finish
`match_static` becomes two steps:
1. `plan(ir, config, constants, jev: bool) -> Plan`. It does today's work. With `jev`, a non-strong HTTP initiator goes into `plan.pending` (call point, substituted target, candidates with tags) and is not linked.
2. `Plan::finish(extra_links, extra_unmatched) -> (Matches, Metrics)` groups everything into dependencies.

`match_static` stays as `plan(.., false).finish(vec![], vec![])`, so existing tests keep their shape. Jev sits between the two steps in the async `jev::resolve`.

Alternative: make the whole matcher async. Rejected, because then the offline path would need a runtime for nothing.

### One answer type for both sessions
Both sessions use `A = Option<String>`, with `none = None`. The value is the service name, or the receiver's point id. This means one model type `M: Model<Option<String>>` serves both sessions. The matcher takes a factory:

```rust
pub trait JevFactory {
    type M: Model<Option<String>>;
    fn model(&self, stage: Stage, answers: Answers<Option<String>>) -> Self::M;
}
```

The CLI's factory builds a `JevModel` with that stage's wording. Tests use a scripted model that answers by stage. `Stage` is `Service | Endpoint`, and its name is the log stage (`match-service`, `match-endpoint`).

Alternative: two generic parameters, one per answer type. Rejected as more code for the same behaviour.

### Wording
- Service. Answer: "Which service does this call reach?" Enough: "Do the facts show which service this call reaches?" Next: "Which fact would help most?"
- Endpoint. Answer: "Which endpoint of the picked service does this call reach?" Enough and next follow the same pattern.
- `JevLimits::default()` (0.7 / 0.5).

The wording lives in one `jev/wording.rs`, so the prompt hash changes only when it is edited.

### Task text
Plain lines, built by `jev/task.rs`:

```
call: GET {self._mds_url}/v3/cases/{case_id}
substituted: http://medical-data-service:8000/v3/cases/{case_id}
from: workbench-service, workbench_service/client.py, fetch_case
binding: self._mds_url = settings.mds_url
candidate: <point id> | medical-data-service | GET /v3/cases/{case_id}
```

When there is no candidate, the last line is `candidate: none`. The endpoint task adds `picked service: <name>`. Rows reuse `agentic-tools`' `row`, which becomes `pub`.

### Answer sets
- Service: config names with a receiver of the protocol, deduplicated and sorted. The description is `<n> <protocol> receivers`.
- Endpoint: receivers of the service with the protocol, and the method when known. They are sorted by `(address_key, id)` and labelled `e1..`. When more than 254 qualify, keep the top 254 by `normalized_levenshtein(path, target path)`, then re-sort them. 254 is `MAX_CHOICE_OPTIONS - 1`, which leaves room for `none`.

### Bindings by point id
`agentic-cli/extract.rs` returns `Extracted { project, ir, bindings: HashMap<PointId, OperandBindings> }`. Each key is built by zipping `ir.restcalls` with the bindings and taking `CodeElement::RestCall(rc.clone()).point_id()`. When two ids are equal, the first wins. Equal ids mean the same file, function, target and span, so the bindings are the same.

### Evidence renumbering
The facts from the service session, then those from the endpoint session, are renumbered `f1..` into one vector. Answers never cite fact ids, so renumbering loses nothing.

### Runtime
- `agentic-cli` uses `#[tokio::main]`.
- Sessions run one after another, in point id order. This keeps the log order stable and stays within the Jev rate limits.
- The log is `Log::open(out/match/sessions.jsonl)`, opened after `create_dir_all`.
- `TypeSafeClient::new()` is called before extraction, so a missing key fails early.

### Fuzzy removal
Removed:
- `fuzzy_fallback`, `fuzzy_distance`, `DISSIMILARITY_PERCENT` and `MatchTag::Fuzzy`
- the `levenshtein` import

`strsim` stays, for the endpoint cap.

### Static evidence
`group::link` takes the `MatchTag`, and writes the `static` fact:
- id `f1`
- args: the tag in lower case
- text: `<service> | <address> <- <target>`

## Risks / Trade-offs
- [Service dir differs from DDFA's. The tools use `assign_service`, DDFA uses `file.contains(base_dir)`.] -> Accepted; the matcher uses `assign_service` everywhere.
- [Jev answers stay at the service level too often, which gives receiver-less links] -> Visible in the `jev` metrics. The oracle is service-level, so the score is not hurt.
- [A run with `--jev` is not reproducible without its log] -> The log is part of the output; a rerun replays it.
- [Static arm loses the fuzzy edge (0.909 on the old config, 0.938 with `http://mds`)] -> Expected. The Jev arm must win it back through `similar` and `service:`. The empaia Jev score is measured in tasks, not promised in specs.
- [Cost: two sessions per Jev call] -> The service session often settles it; the endpoint session's answer set is small.

## Open Questions
None.
