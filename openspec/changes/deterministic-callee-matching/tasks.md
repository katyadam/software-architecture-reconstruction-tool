# Tasks

## 1. Fix

- [x] 1.1 Add a statix test where two Python `base` functions (`-> str` and untyped) match `base()`; evaluate 64 times with fresh maps and assert one result. Verify it fails before the fix (`cargo test -p statix --test mod python_callee`)
- [x] 1.2 In `find_closest_callable_impl`, break ties between exact matches and between equal fallback scores by the smallest key; verify `cargo test -p statix` passes
- [x] 1.3 Verify downstream crates still pass: `cargo test -p extractor-runtime` and `cargo test -p python-extractor`

## 2. Review follow-up

- [x] 2.1 Rank candidates in one pass by arity, matched types, then smallest key (exact match = all types match, header parsed once); add a test that `f(unknown)` resolves to `f(x)`, not `f()`, and verify it fails before and passes after (`cargo test -p statix --test mod python_call`)
