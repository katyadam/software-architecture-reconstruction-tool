## 1. Shared types

- [x] 1.1 `agentic-models`: derive `PartialOrd, Ord` on `Protocol`. Verify `cargo test -p agentic-models`.
- [x] 1.2 ~~`models::interaction_kind`: add `is_test_file`~~ Dropped (2026-11-21): no non-HTTP link exists yet, so the non-HTTP kind rule was dead code. It comes with the first non-HTTP adapter. `models` is unchanged.

## 2. Baseline snapshot

- [x] 2.1 Before any matcher edit, run `agentic-cli` on empaia (flags as in 5.1) into the scratchpad directory as the reference. Verify `final/sdg.legacy.json` scores 0.941/0.941/0.941.

## 3. Points

- [x] 3.1 Rename `Placed` to `Point`, move it to `points/mod.rs` and drop `method` and `target`. The HTTP adapter reads the method and endpoint uri from the element. Verify `cargo test -p agentic-matcher`.
- [x] 3.2 `points/message.rs`: `MessageEdge` -> `Address` (`Rpc` split on the first `/`, `Channel{name, broker: exchange}`, `Unresolved` for an empty destination). Tests: gRPC with and without a method, RabbitMQ with an exchange, empty destination. Verify the tests pass.
- [x] 3.3 `points::build(ir, config, substituter) -> Points`: calls and producers are initiators, endpoints and consumers are receivers, bindings and declarations are counted as topology per protocol. Unassigned receivers are kept under the empty service. Test: one producer, one consumer and one binding give one initiator, one receiver and topology 1. Verify the test passes.

## 4. Matching core

- [x] 4.1 Rename `candidates/` to `http/`. Move `request_kind` from `group.rs` into it. The call target is substituted once per initiator. Verify the copied tests in `http/tests.rs` pass unchanged apart from their helpers.
- [x] 4.2 `lib.rs`: `match_static(ir: &EvaluatedIR, config, constants)` dispatches with a `match` on the address: `Http` -> `http::candidates` over same-protocol receivers, `Unresolved` -> `AddressUnresolved`, `Rpc`, `Channel` and `Resource` -> `NoCandidate` with the note `no rules for <protocol>`. Test: one Kafka producer becomes `NoCandidate` with that note. Verify the test passes.
- [x] 4.3 `group.rs`: dependencies keyed by `(source, target, protocol)`. Test: an HTTP link and a RabbitMQ link between the same two services give two dependencies. Verify the test passes.
- [x] 4.4 `metrics.rs`: `BTreeMap<Protocol, ProtocolMetrics>` with the spec's per-protocol counts (including `unresolved` and `topology`), plus total `dependencies`. Update the tie test to the HTTP entry. Verify `cargo test -p agentic-matcher` and `cargo clippy -p agentic-matcher --all-targets --no-deps -- -D warnings`.

## 5. CLI and acceptance

- [x] 5.1 `agentic-cli` passes the evaluated IR. `legacy.rs` keeps only HTTP dependencies. Verify `cargo test -p agentic-cli` and `cargo clippy -p agentic-cli --all-targets --no-deps -- -D warnings`.
- [x] 5.2 Run `agentic-cli -p ../empaia -c config/configurations/local-empaia-config.json -f config/constants/empaia-constants.json -o results/agentic-empaia-points`. Score `final/sdg.legacy.json` with the zenodo score.py. Verify 0.941/0.941/0.941 with the same FP and FN, and `diff` it against the 2.1 reference legacy file with no difference.
- [x] 5.3 Run 5.2 a second time into another directory and `diff -r` the two. Verify no difference.
- [x] 5.4 Run `agentic-cli -p ../train-ticket -c config/configurations/local-train-ticket-config.json -o results/agentic-tt-points`. Verify `metrics.json` has a RabbitMQ entry and `unmatched.json` holds RabbitMQ points. Record both runs' per-protocol counts here. Run `openspec validate matcher-points --strict` and verify no errors.
  - empaia, 2026-11-21: Http 577 initiators (126 no candidate, 434 single, 15 same-service tie, 2 cross-service tie), 1210 receivers, 476 links. Kafka 4 initiators, all no rules, 0 receivers. 17 dependencies. Score TP 16 FP 1 FN 1, 0.941. Legacy file identical to the 2.1 reference. Two runs byte-identical.
  - train-ticket, 2026-11-21: Http 237 initiators (6 no candidate, 226 single, 5 same-service tie, 0 cross-service tie), 264 receivers, 236 links. RabbitMq 4 initiators, all no rules on `Queues.queueName`, 2 receivers, 5 topology. 91 dependencies.
  - Review fixes, 2026-11-21: message destinations substituted, `no_rules` counter, explicit dependency target, `From<&CommunicationProtocol>`, `EvaluatedIR: Default`. Reran 5.2–5.4: same score, legacy file still identical to 2.1, runs byte-identical.
