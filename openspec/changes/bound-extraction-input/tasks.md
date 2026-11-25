# Tasks

## 1. Depth limit

- [x] 1.1 Add `MAX_TREE_DEPTH` and iterative `tree_depth_exceeds` to `statix/src/util.rs` (verify: builds)
- [x] 1.2 Reject over-deep trees in Go, Python and Java `extract_syntactic` (verify: `bounded_input` deep test passes; crashes without the change)

## 2. Size limit

- [x] 2.1 Skip files over `MAX_SOURCE_BYTES` (4 MiB) in `dispatch_syntactic` with a warning (verify: `oversized_file_is_skipped` passes)

## 3. Verification

- [x] 3.1 Regression tests in `extractor-runtime/tests/pipeline/bounded_input.rs`, including a just-under-limit case per shape on a 2 MiB thread (verify: `cargo test -p extractor-runtime --test mod bounded_input`)
- [x] 3.2 fmt, clippy and tests for touched crates pass
