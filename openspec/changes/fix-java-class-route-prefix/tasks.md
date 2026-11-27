# Tasks

## 1. Annotation parsing

- [x] 1.1 Add top-level argument parsing in `endpoints/annotations.rs` (`value`/`path`/unnamed, arrays, `method=`). Verify: B3 test passes.

## 2. Per-class prefix

- [x] 2.1 In `endpoints/extractor.rs` resolve the enclosing type of each method, take prefixes only from `@RequestMapping`/`@Path`, skip `@FeignClient` types, emit one endpoint per prefix; remove the `class_annotation` query pattern. Verify: B1, B2, B4, B6 tests pass.

## 3. Verification

- [x] 3.1 Add one regression test per case (B1 x3, B2, B3, B4, B6) in `java-extractor/tests/java/endpoints.rs`; run `cargo test -p java-extractor`, `cargo clippy -p java-extractor --all-targets`, `cargo fmt --all --check`.
