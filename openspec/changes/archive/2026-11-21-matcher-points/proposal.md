## Why
The agentic models and architecture are protocol-agnostic, but the `matcher-static` code is REST-shaped. `match_static` takes `&[Endpoint]` and `&[RestCall]`. `Placed` copies the HTTP method and URL. Dependencies are grouped by `(source, target)` with `Protocol::Http` hardcoded. Metrics have no protocol. Train-ticket has 7 RabbitMQ connections in the baseline, all joined on the unresolved name `Queues.queueName`. Fixing the core now is cheap. Once `facts-tools` and `matcher-jev` build on `Placed`, it gets expensive.

## What Changes
- Points come from every extracted element: REST calls, endpoints and message edges. Producers become initiators and consumers become receivers. Bindings and queue/topic declarations do not become points; they are counted as topology for a later resolution slice.
- A message point gets `Address::Rpc` (gRPC) or `Address::Channel` (Kafka, RabbitMQ) from its edge.
- `Placed` becomes `Point`, keeping only the connection point and its service. The HTTP adapter reads the method and path from the address and substitutes the call target itself.
- `match_static` takes the evaluated IR, not REST slices. **BREAKING** for the crate API; `agentic-cli` is the only caller.
- Each initiator is matched by the adapter for its address: `Http` -> today's candidate rules. `Rpc` and `Channel` have no adapter yet and become `Unmatched` with reason `NoCandidate`. `Unresolved` becomes `Unmatched` with reason `AddressUnresolved`.
- Dependencies are grouped by `(source, target, protocol)`, the protocol taken from the points.
- HTTP kinds stay on `classify`. Non-HTTP link kinds come with the first non-HTTP adapter, since no non-HTTP link can exist before it.
- `metrics.json` counts per protocol. **BREAKING** for the file shape.
- `sdg.legacy.json` takes only HTTP dependencies. `message_connections` stays empty.
- No new adapter, no model, no Ollama. Empaia parity must hold.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `agentic-static-matcher`: points from all protocols, adapter dispatch by address, protocol in dependency grouping, per-protocol metrics, HTTP-only legacy output.

## Impact
- `agentic/matcher`: `lib.rs`, `points.rs` (+ message addresses), `candidates/` becomes the HTTP adapter, `group.rs`, `metrics.rs`.
- `agentic/cli`: passes the evaluated IR; `legacy.rs` filters HTTP.
- `agentic-models`: `Protocol` derives `Ord` for sorted grouping and metrics.
- Output: `metrics.json` shape changes. `unmatched.json` now also holds message points.
