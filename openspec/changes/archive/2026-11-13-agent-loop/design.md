## Context

- DDFA (`extractor-runtime/.../llm_enhance/ddfa.rs`, `resolve`) already runs this loop for Jev. It offers untaken actions as a `choice`, fetches the picked one, and decides at budget or at "enough". Its rules carry over. Its types do not: they are tied to `RestCall` and `SystemOneRequest`.
- Jev answers typed questions (`choice`, `noul`, `score`) and never free JSON. System two answers JSON under a schema. The loop therefore speaks in `Step`s, and each adapter maps its provider's reply to a `Step`. Adapters come in `agent-loop-adapters`.
- `agentic_models::Fact { id, tool, args, text }` and `FactId` already exist.

## Goals / Non-Goals

**Goals:**
- One loop for matcher, verifier and rematcher, testable with a scripted fake model.
- Budget, citation and error rules enforced by the runtime, not trusted to the model.
- A crash costs at most the sessions in flight.

**Non-Goals:**
- Provider adapters, context size, token-mismatch check (`agent-loop-adapters`).
- `Context`/`ProjectIR`, public `llm_enhance/facts` tools (`matcher-http`).
- Concurrency control. The log is safe to share, but the loop does not schedule sessions.

## Decisions

**Types** (crate `agent-loop`: `protocol.rs` model and tool types, `session.rs` the loop, `log.rs` log and hash, `lib.rs` re-exports only):

```rust
pub struct ToolOption { pub id: String, pub tool: String, pub args: String, pub description: String }

pub struct Request<'a> {
    pub task: &'a str,            // stage prompt for this point
    pub facts: &'a [Fact],        // recorded so far
    pub options: &'a [ToolOption] // empty -> must answer
}

pub enum Step<A> { Call(String /* option id */), Final { answer: A, cites: Vec<FactId> } }

pub struct Reply<A> { pub step: Step<A>, pub input_tokens: u64, pub output_tokens: u64 }

pub trait Model<A> {
    fn id(&self) -> &str;
    async fn step(&mut self, req: &Request<'_>) -> Result<Reply<A>, String>;
}

pub trait Tools {
    fn available(&self, taken: &BTreeSet<String>) -> Vec<ToolOption>;
    fn run(&self, option: &ToolOption) -> String;
}

pub enum Outcome<A> { Answer { answer: A, cites: Vec<FactId> }, BudgetExhausted, ModelError(String) }

pub struct Session<A> { pub outcome: Outcome<A>, pub facts: Vec<Fact>, pub calls: usize,
                        pub input_tokens: u64, pub output_tokens: u64, pub replayed: bool }

pub async fn run_session<A, M: Model<A>, T: Tools>(
    key: &SessionKey, task: &str, model: &mut M, tools: &T, budget: usize, log: &Log,
) -> Session<A>;
```

- `A` is the stage's answer type (matcher outcome, verifier decision). It needs `Serialize + DeserializeOwned` for the log.
- `SessionKey { stage: String, subject: String }`. The subject is a `PointId` (matcher, rematcher) or a `LinkId` (verifier), rendered as a string. One string key avoids a generic or an enum used only for lookup.
- `Model::step` returns `Result<_, String>`. Adapters stringify provider errors. A typed error enum would have one consumer that only prints it.
- Native `async fn` in traits (edition 2024). No `dyn`, so no `async-trait`.

**Loop.**
1. Look up the log first. On a hit, return the logged session with `replayed: true`.
2. Then loop:
   - Offer options with `available(&taken)`, but offer an empty list once `facts.len() == budget`.
   - On `Call(id)`: find `id` in the offered list, or end with `ModelError`. Then run the tool, push `Fact { id: f{n}, tool, args, text }`, and insert `id` into `taken`.
   - On `Final`: every cite must be in `facts`, else `ModelError`.
   - A `Call` when no options were offered is `BudgetExhausted`, whether the budget is spent or the tools ran out.
- A step budget beats a token budget: DDFA uses steps, and the architecture fixes 6 for the matcher and 0/3 for the verifier.

**Prompt hash.**
- `sha256(model id \0 budget \0 task)`, as hex. NUL separators keep parts from running into each other. The budget is included so a verifier escalating from 0 to 3 steps does not replay its budget-0 answer. `sha2` is already a workspace dependency.
- Tool options are left out. They are derived from the code, which the task already identifies. Hashing them would rerun a point whenever option order changes.

**Log.**
- `Log` wraps `Mutex<File>` (append mode) plus an index `HashMap<(stage, subject, hash), Record>` loaded at open.
- One `serde_json` line per record, written with `writeln!`, then `flush` while still under the lock.
- `Record` holds stage, subject, prompt_hash, facts, outcome (`Answer` or `BudgetExhausted`), calls and tokens.
- Records are generic over `A` via serde. A malformed line at load is skipped with a warning to stderr. A crash can leave the last line half-written.
- Alternative considered: one file per point. Rejected because thousands of small files make spread comparison across runs harder.
- **Model errors are not logged**, so resume retries them.

## Risks / Trade-offs

- A cached answer survives a change to the tools' code. Delete the log, or bump the task text, to force a rerun. -> Accepted. The task text carries the stage prompt version.
- The `Mutex` serializes log writes. -> Negligible next to model latency.
- `Model` takes `&mut self`, so one model instance serves one session at a time. -> Fine while sessions are sequential. Revisit with concurrency.
- The loop trusts `Tools::run` to bound fact size. DDFA caps facts at 40 lines. -> Each `Tools` impl caps its own output.
