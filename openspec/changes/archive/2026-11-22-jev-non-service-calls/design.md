## Context

See proposal.md, Why. Today `Pick = Option<String>`: `Some(name)` is a service (or a receiver id in the endpoint stage), and `None` is `none`. `Answers<A>` is generic over the value, and `final_answer` in `agent-loop` maps the picked label to its value. `Matches.excluded` and `ExcludedReason::External` already exist, but nothing fills them: `Plan::finish` writes `excluded: vec![]`.

## Goals / Non-Goals

**Goals:**
- One more answer kind in the service stage, with no new session and no new model call.
- Reuse `Excluded` instead of a new outcome type.

**Non-Goals:**
- Changing `agent-loop`. The adapters stay generic over the answer value.
- Changing the endpoint stage's answers.

## Decisions

**1. `Pick` becomes `Option<Target>`.**
```rust
pub enum Target { Point(String), External, InProcess }
pub type Pick = Option<Target>;
```
`Point` holds a service name in the service stage and a receiver id in the endpoint stage, as `String` does today. `None` stays `none`, so `Answers::none` and the threshold fallback in `final_answer` don't change.
- Alternative: sentinel strings (`"__external"`) inside `Option<String>`. Rejected: a service could be named that way, and every match would need a string compare.
- Alternative: two `Pick` types, one per stage. Rejected: `JevFactory::model` takes one answer type, and splitting it doubles the factory for one extra variant.

**2. The two answers are plain options at the end of `answers::services`.** They have labels `external` and `in-process`, and their descriptions carry the definition. `questions::distinct` already rejects duplicate labels, so a service with one of these names fails the session as a model error. The spec asks for exactly that.

**3. The wording sits in two places.** The option descriptions say what each kind covers. The service `answer` question in `wording.rs` adds the two rules: a database layer, cache, dict or SDK call is not a service call; own service only for an HTTP call to its own endpoints. The task text gets only facts about the call (decision 6).

**4. `Resolved` gains `Excluded(Box<Excluded>)`.** `outcome::resolve` maps `Target::External` and `Target::InProcess` straight to it, with the renumbered facts and `note = "confidence {p}; candidates: <ids>"`; the candidates part is left out when there are none. `one()` in `jev/mod.rs` runs the endpoint session only for `Target::Point`. `JevResult` gains `excluded`, and `JevMetrics.count` adds `excluded: BTreeMap<String, usize>` by reason name, the same as `unmatched`.

**5. `Plan::finish(links, unmatched, excluded)`** writes the excluded points into `Matches`. They are already in point id order, because `pending` is sorted and Jev keeps that order. The CLI output writes `match/excluded.json` next to `unmatched.json`. The legacy SDG is unchanged, because excluded points never reach it.

**6. The task shows the call's source text.** The first run (114 -> 73 own-service picks) showed why Jev still picks own service: the task held only the raw target, e.g. `collection_id`, so the route the call sits in looked like a perfect match. The facts `Subject` now reads the call expression from its `call_span`, the same call site it already finds for the snippet. `PointTools::call` exposes it, and the service task adds a `code:` line, flattened to one line and cut at 300 characters. DDFA's own `Subject` sets it to `None`, so its prompts don't change.
- Alternative: an `IS_HTTP` triage gate. Rejected for now: one more call per initiator, and it doesn't fix what the service session sees.

## Risks / Trade-offs

- [Jev over-uses `in-process` and drops real calls] -> Check the 17 oracle edges and the excluded list in the next run. Any excluded point with a host of a configured service is a regression to read by hand.
- [The prompt changes, so replay is lost] -> One fresh empaia `--jev` run, about $0.24. The old run stays in `jev_triage_runs/` for comparison.
- [`Excluded` has no confidence field] -> The note carries it. A real field waits until the verifier needs it.

## Migration Plan

None. The new output file is additive. Old session logs won't replay because the prompt hash changes, which is expected.
