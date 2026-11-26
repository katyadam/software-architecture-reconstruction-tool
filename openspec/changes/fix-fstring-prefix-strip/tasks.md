# Tasks

## 1. Fix

- [x] 1.1 Add regression tests in `uri_generator.rs`: `HOST = "frontend/{x}"` with `HOST + "/y"` yields `frontend/{x}/y`; `f"frontend/{id}"` yields `frontend/{id}`. Verify the first test fails before the fix
- [x] 1.2 In `resolve_fstring`, strip the leading `f` only when a quote follows it; verify `cargo test -p python-extractor` passes
- [x] 1.3 Review follow-up: add a test that an already-unquoted target `files/{id}` resolves unchanged, and stop routing plain `f...` values without `{` to `resolve_fstring`; verify `cargo test -p python-extractor` passes
