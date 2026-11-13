## 1. Types

- [x] 1.1 Add `Decision` and `Verdict` to `agentic/models/src/lib.rs`. Verify: a test round-trips a `Drop` verdict through JSON.
- [x] 1.2 Add `LinkId { initiator, target: LinkTarget }` with `LinkTarget { Receiver(PointId), Service(ServiceId) }`, and `Dependency::link_id(&Link) -> LinkId`. Verify: tests cover pair, service-only, and two receivers of one initiator giving different ids.
- [x] 1.3 Add `Location { file, span: Option<SourceSpan> }` and `ConnectionPoint::location()`. Verify: tests cover REST call with span, REST call with empty span, endpoint.

- [x] 1.4 Add the HTTP method to endpoint point ids (`<METHOD> <uri>`). Verify: a test gives `GET /x` and `POST /x` in one function different ids, matching the spec scenario.
- [x] 1.5 Move `location()` onto `CodeElement`; `point_id()` reuses it for file and span, `ConnectionPoint::location()` delegates. Verify: one copy of the empty-span rule; all crate tests pass.
- Deferred from review: message ids ignore routing key and topic (channel work); duplicate service-only links from one initiator (`matcher-http` must not emit them).

## 2. Workspace

- [x] 2.1 Replace `"agentic/models"` with `"agentic/*"` in root `Cargo.toml` members. Verify: `cargo metadata --no-deps` lists `agentic-models`; `cargo test -p agentic-models` passes.

## 3. Close out

- [x] 3.1 Run `cargo fmt --check` and `cargo clippy -p agentic-models --tests` on the crate, then `openspec validate agentic-models-verdicts --strict`. Verify: no errors, no new warnings.
