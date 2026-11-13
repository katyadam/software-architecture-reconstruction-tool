## 1. Check built code against spec

- [x] 1.1 Compare `agentic/models/src/lib.rs` with each requirement in `specs/agentic-matches-model/spec.md`; list mismatches in this task, or mark none. Verify: every requirement maps to a type or method.
  - All 10 requirements map to types or methods. No type mismatch.
  - Not enforced by the crate, left to stage writers: kind rollup (`Business` wins), `element` immutability, `ModelError` on provider failure, `External` exclusion. Matcher (`matcher-http`) and agent-loop must enforce them.
- [x] 1.2 Add a test that a gRPC `MessageEdge` gives `Protocol::Grpc` and a RestCall gives `Http`. Verify: `cargo test -p agentic-models` passes.
- [x] 1.3 Confirm synthesizer uses `models::InteractionKind` with no own copy. Verify: `grep -rn "enum InteractionKind"` finds only `models/src/interaction_kind.rs`; `cargo test -p synthesizer` passes.

## 2. Close out

- [x] 2.1 Run `openspec validate agentic-architecture --strict`. Verify: no errors.
- [x] 2.2 Scaffold the next change `agentic-models-verdicts` with `openspec new change`. Verify: it appears in `openspec list`.
