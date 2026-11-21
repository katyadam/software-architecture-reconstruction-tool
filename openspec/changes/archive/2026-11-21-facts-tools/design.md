## Context
The architecture (`2026-11-13-agentic-architecture/design.md`, "Tool protocol") lists the tools. `agent-loop` defines the trait:

```rust
pub trait Tools {
    fn available(&self, taken: &BTreeSet<String>) -> Vec<ToolOption>;
    fn run(&self, option: &ToolOption) -> String;
}
```

The DDFA facts (`llm_enhance/facts`, about 750 lines with tests) read four residual-only inputs:

| Field | Used for |
|---|---|
| `rc.file_path`, `rc.function_hash` | enclosing callable -> `callers` |
| `rc.target_uri` | `mentioned()` -> `open:` order |
| `bindings` | `symbols()` -> `define:`, `constant:`; `mentioned()` |
| `state.receiver.datatype`, `state.enclosing.class` | `receiver` |
| `state.enclosing.function` | `callers` option text |
| `state.snippet` | `mentioned()` |

Everything else (`project_ir`, `config`, `external_constants`, `origin`, `service_dir`) is already neutral.

`evaluate()` takes `ProjectIR` by value but only borrows it inside. The `(RestCall, OperandBindings)` pairs exist inside it and are dropped.

## Goals / Non-Goals

**Goals:**
- One fact implementation shared by DDFA and the agentic tools.
- Tools for any initiator protocol.
- DDFA and baseline output byte-identical.

**Non-Goals:**
- Prompts, sessions, Jev, candidates as a closed answer set. These come in `matcher-jev`.
- Removing `fuzzy_fallback`, which is `matcher-jev` too.
- Receiver-side tools (`--both-sides`) and verifier tools.
- Spans on endpoints or message edges.

## Decisions

### Neutral `Subject`
```rust
pub struct Subject {
    pub file: String,
    pub function_hash: String,
    pub enclosing_function: String,   // option text for `callers`
    pub enclosing_class: Option<String>,
    pub receiver_type: Option<String>,
    pub symbols: BTreeSet<String>,
    pub mentions: Vec<String>,        // target, snippet, binding values
}
impl Subject {
    pub fn rest_call(rc: &RestCall, bindings: &OperandBindings, ir: &ProjectIR) -> Self;
    pub fn message(edge: &MessageEdge, ir: &ProjectIR) -> Self;
}
```
- The call-site lookups (`source_call`, `read_span`, `strip_args`, `relative_file`) move to `pipeline/facts/site.rs`. `Subject::rest_call` and `residual_call_state` both use them.
- DDFA builds its subject from its prompt state (`residual_edge_filter::subject`), so its facts read exactly what its prompt shows. A test checks that `Subject::rest_call` gives the same subject for the same call.
- `symbols()` stays as it is, over the binding values. For a message, it runs over the raw destination: `Queues.queueName` gives `queueName`.
- `FactCtx { project_ir, config, external_constants, subject, origin, service_dir }`.

Alternative: a `trait FactSubject` implemented by the REST and message contexts. Rejected: there are only two constructors and no behavior to vary.

### Module move
`pass3/llm_enhance/facts/` moves to `pipeline/facts/`, as `pub mod facts`. Only `fetch` changes shape: it returns `(id, body)`, and DDFA wraps that into its `clients` `Fact`. `available` no longer truncates; DDFA keeps its `MAX_CHOICE_OPTIONS - 1` cap. The test fixtures split in two:
- the project, config and `Subject` go to `pipeline/facts/test_fixtures.rs`, behind `cfg(any(test, feature = "test-fixtures"))`, so `agentic-tools` tests reuse them
- the DDFA `rc`, bindings and state stay in `llm_enhance`

### `evaluate` borrows
```rust
pub fn evaluate(ir: &ProjectIR, ...) -> EvaluatedIR
pub fn evaluate_with_bindings(ir: &ProjectIR, ...) -> (EvaluatedIR, Vec<OperandBindings>)
```
`bindings[i]` belongs to `restcalls[i]`. `evaluate` returns `.0`. `OperandBindings` becomes `pub`. `evaluate_with_llm` keeps owning its IR, so nothing changes there.

### Crate `agentic/tools`
```rust
pub struct ToolCtx<'a> {
    pub project: &'a ProjectIR,
    pub config: &'a ConfigurationData,
    pub constants: &'a HashMap<String, String>,
    pub receivers: &'a [&'a ConnectionPoint],   // all protocols
}
pub struct PointTools<'a> { ctx, point, subject, origin, explored: RefCell<Explored> }
impl<'a> PointTools<'a> {
    pub fn new(ctx: &'a ToolCtx, point: &'a ConnectionPoint, bindings: Option<&OperandBindings>) -> Option<Self>;
}
impl Tools for PointTools<'_> { ... }
```
- `new` returns `None` when the point's file is not in the IR. `matcher-jev` then runs with no code facts.
- Modules:
  - `lib.rs`: ctx and dispatch
  - `code.rs`: the `pipeline/facts` bridge
  - `receivers.rs`: `similar` and `service:`
  - `imports.rs`
  - `tests.rs`
- A new crate, not a matcher module, because the verifier needs the same tools. The matcher stays free of `extractor-runtime` until `matcher-jev`.

Alternative: a module in `agentic/matcher`. Rejected: the verifier would depend on the matcher only for its tools.

### `Explored` behind `RefCell`
`Tools::run` takes `&self`, but opening a file must widen the `open:` options. `PointTools` keeps `Explored` in a `RefCell`. `available` filters by the session's `taken` set, which agrees with `Explored.taken`.

Alternative: `run(&mut self)` in `agent-loop`. Rejected: it changes a merged spec for one caller.

### `similar` ranking
The address key is `METHOD path` for HTTP, `service/method` for RPC and `broker:name` for a channel. The score is normalized Levenshtein from `strsim`, which is already a workspace dependency. The rank orders by `(distance, service, id)`, and the top 10 are kept.

Alternative: reuse `http::fuzzy_distance`. Rejected: it is HTTP-only and leaves with `fuzzy_fallback`.

### Option ids
| Id | Args |
|---|---|
| `define:<name>`, `constant:<name>`, `callers`, `receiver`, `open:<rel>#<cw>` | as DDFA |
| `similar` | none |
| `imports` | none |
| `service:<name>` | config service name |

Order: code facts, then `similar`, then `imports`, then `service:` by name. `service:` is offered only for services with a receiver of the initiator's protocol, so no option is sure to find nothing. `imports` lists importers in the origin's language only. DDFA truncates its options to `MAX_CHOICE_OPTIONS - 1` for Jev's choice size. `PointTools` does not truncate; `matcher-jev` decides the limit.

### Rows
`<point id> | <service> | GET /v3/cases`. These are rendered through the shared 40-line `render`.

## Risks / Trade-offs
- [Service directory rule differs from DDFA] -> the tools use `assign_service`, the shared rule. DDFA uses `file.contains(base_dir)`, so an odd layout can scope the two differently. Accepted.
- [Bindings are aligned by index with `EvaluatedIR.restcalls`] -> `matcher-jev` keys them by point id before building points. Otherwise REST tools get no `define:` or `constant:` options.
- [DDFA drift from the move] -> the existing facts tests stay and must pass unchanged. A new test compares `Subject::rest_call` with the old `ResidualCallState` fields on the fixture.
- [Point ids are long, so rows are wide] -> accepted. `matcher-jev` may add short aliases if tokens hurt.
- [`similar` on unresolved paths ranks noise] -> it is a tool, not a decision. `matcher-jev` filters candidates itself.
- [`extractor-runtime` pulls actix into the agentic crates] -> already accepted in the architecture.
- [Message subjects have no bindings] -> symbols come from the raw destination only. A real binding source can come with the first message adapter.

## Migration Plan
None. The API change is internal: the `evaluate` callers add `&`.
