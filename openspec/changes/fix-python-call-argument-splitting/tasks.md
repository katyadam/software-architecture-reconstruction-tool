# Tasks

## 1. Argument building

- [x] 1.1 Build `Argument`s in `calls/extractor.rs` from the `argument_list` node's named children (keyword_argument name/value, skip comments, positional otherwise); verify with a calls test covering `?`/`=`, `,` in strings and keyword arguments

## 2. REST call target

- [x] 2.1 In `restcalls/identification/method_call.rs` take the target from the `url` keyword, else the first argument; verify with an `extractor-runtime` pass-3 regression test covering all three cases from #59

## 3. Verification

- [x] 3.1 Run `cargo test -p python-extractor -p extractor-runtime`, `cargo clippy -p python-extractor -p extractor-runtime --all-targets`, `cargo fmt --all --check` and verify they pass
