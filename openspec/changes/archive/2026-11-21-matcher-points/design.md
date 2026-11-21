## Context

- `matcher-static` (archived 2026-11-21) matches REST only. The REST shape is in five places: the `match_static` signature, `Placed.method` / `Placed.target`, `group::dependencies` (`(source, target)` key, `Protocol::Http` hardcoded), HTTP-only `classify` signals, and protocol-free metrics.
- `agentic-models` is already generic: `ConnectionPoint{side, protocol, address, element}`, `Address::{Http, Rpc, Channel, Resource, Unresolved}`, `CodeElement::Message(MessageEdge)`. `Dependency` is documented as keyed by `(source, target, protocol)`.
- `EvaluatedIR` holds `endpoints`, `restcalls` and `message_edges`.
- `MessageEdge` has `protocol` (`RabbitMq | Kafka | Grpc`), `role` (`Producer | Consumer | QueueDeclaration | TopicDeclaration | Binding`), `destination`, `exchange`, `routing_key`, `queue`, `topic`. For gRPC, `destination` is `Service[/Method]`.
- The synthesizer pairs messages with three strategies (`message_connection_strategies.rs`): RabbitMQ topology via bindings, direct destination join, binding fallback.
- Train-ticket baseline: 7 RabbitMQ connections, all on the unresolved destination `Queues.queueName`. Empaia: no message edges.

## Goals / Non-Goals

**Goals:**
- One neutral core: build points, dispatch per address kind, group, classify, count. REST becomes one adapter.
- Empaia parity and byte-identical runs kept.
- Message points visible in `unmatched.json` and the metrics.

**Non-Goals:**
- RPC or channel matching rules. RabbitMQ topology resolution. Both are later slices.
- Scoring non-HTTP edges. No oracle has them.
- `message_connections` in `sdg.legacy.json`.
- `Resource` addresses.

## Decisions

### Point keeps only the point and its service
```rust
pub struct Point {
    pub point: ConnectionPoint,
    pub service: ServiceDescription,
}
```
`Placed` is renamed `Point`; `method` and `target` go. The HTTP adapter reads the method from the element. An endpoint's target is `element.uri`. A call's target is the substituted URL, computed once per initiator in the adapter with the shared `Substituter`.

Alternative: put the substituted URL in `Address::Http`. Rejected: it changes `agentic-matches-model`, and `Address::Http` already splits host and path. Rebuilding the URL from them loses the scheme and port that `is_reflexive` compares.

Alternative: a generic `target: String` on `Point`. Rejected: it is an HTTP field with a neutral name.

### Points from all elements
```
points::build(ir: &EvaluatedIR, config, substituter) -> Points {
    initiators: Vec<Point>,
    receivers: Vec<Point>,
    topology: BTreeMap<Protocol, usize>,
}
```
- RestCall -> initiator, `Address::Http` as today.
- Endpoint -> receiver, `Address::Http` as today.
- MessageEdge, by role:
  - `Producer` -> initiator
  - `Consumer` -> receiver
  - `Binding`, `QueueDeclaration`, `TopicDeclaration` -> topology count only
- Message address (`points/message.rs`):
  - empty `destination` -> `Unresolved{raw: destination}`
  - gRPC -> `Rpc{service, method}`, split on the first `/`; no `/` -> empty method
  - Kafka, RabbitMQ -> `Channel{name: destination, broker: exchange}`
- Service assignment as today: unassigned initiators skipped, unassigned receivers kept under the empty service. Both are counted per protocol.

`match_static(ir: &EvaluatedIR, config, constants)`. `agentic-cli` passes the IR it already has.

### Dispatch with a match, not a trait
```rust
match &call.point.address {
    Address::Http { .. } => http::candidates(call, &target, &same_protocol, &all_urls),
    Address::Unresolved { .. } => unresolved,
    Address::Rpc { .. } | Address::Channel { .. } | Address::Resource { .. } => no_rules,
}
```
- Receivers are pre-filtered to the call's protocol.
- `no_rules` -> `Unmatched{reason: NoCandidate, note: "no rules for <protocol>"}`, counted as `no_rules`, apart from real misses.
- `unresolved` -> `Unmatched{reason: AddressUnresolved}`.

Alternative: a `ProtocolMatcher` trait. Rejected: there is one implementation. Add the trait when a second adapter lands, if the match grows past one line per arm.

### Module layout
```
agentic/matcher/src/
  lib.rs              match_static: build -> dispatch -> group -> metrics
  points/mod.rs       Point, build, assign_or_log
  points/message.rs   MessageEdge -> Address
  http/mod.rs         was candidates/mod.rs; + request_kind (moved from group.rs)
  http/segments.rs    unchanged
  http/tests.rs       unchanged, helpers build Point
  group.rs            link, no_candidate, unresolved, dependencies, kind
  metrics.rs          per-protocol counts
```

### Grouping by protocol
The dependency key becomes `(source, target, protocol)`, with the protocol taken from the initiator point. `Protocol` derives `PartialOrd, Ord` in `agentic-models`, so the `BTreeMap` order is deterministic. The derive order is `Http, Grpc, Kafka, RabbitMq`.

### Kinds
- HTTP: `http::request_kind`, unchanged (`classify`).
- Other protocols: no rule yet. No non-HTTP link can exist before an RPC or channel adapter, so a rule now would be dead code. The first adapter adds it, e.g. `TestOrigin` by the shared test-file rule, else `Business`.

### Metrics shape
```json
{
  "protocols": {
    "Http": { "initiators": 577, "no_candidate": 126, "no_rules": 0, "unresolved": 0, "single": 434,
              "same_service_tie": 15, "cross_service_tie": 2, "receivers": 1210,
              "topology": 0, "unassigned_initiators": 0, "unassigned_receivers": 0, "links": 476 }
  },
  "dependencies": 17
}
```
A `BTreeMap<Protocol, ProtocolMetrics>`. A protocol is present once any point or topology element of it exists.

### Legacy writer
`legacy()` keeps dependencies with `protocol == Http`. Today no other protocol produces links, but the filter keeps score.py input HTTP-only once one does.

## Risks / Trade-offs

- [Parity drifts while moving code] -> no HTTP rule changes. Score empaia and diff a run's `sdg.json` against the `matcher-static` output before and after.
- [gRPC `destination` format differs per extractor] -> only the split on `/` depends on it. A malformed value gives an empty method, not a crash. It is visible in `unmatched.json`.
- [`metrics.json` shape break] -> no consumer exists outside this repo. The archived `matcher-static` metrics stay in its tasks file.
- [Message point ids can collide: `point_id` is file, function hash and destination, and messages have no span. Two publishes to one queue in one function share an id.] -> harmless while message points end in `unmatched`. Fix in the first message adapter, e.g. add the handler or a span.
- [Message receivers are not in `Service.endpoints`] -> the field is `Vec<Endpoint>`. Consumers stay reachable through points. Revisit when message links exist.
