# Tasks

## 1. Fix

- [x] 1.1 Add regression tests in `uri_generator.rs`: 20 two-valued parts yield exactly 256 URIs, the first being all first values; 2 x 3 values still yield 6. Verify the cap test fails before the fix
- [x] 1.2 Add `MAX_TARGET_URIS` and bound each fold step in `generate_target_uris` with a lazy `flat_map(...).take(MAX_TARGET_URIS)`; verify `cargo test -p python-extractor` passes
