# Tasks

## 1. Fix

- [x] 1.1 Add regression tests in `uri_generator.rs` for `"a+" + x` (no panic, one URI) and `"" + "/api"` (yields `/api`); verify the first fails before the fix
- [x] 1.2 Guard the quoted-literal slice in `get_resolved_parts` with `part.len() >= 2`; verify `cargo test -p python-extractor` passes
