# Tasks

## 1. Import alias fix

- [x] 1.1 Add regression test `major_version_import_paths_use_package_name_as_alias` in `go-extractor/src/extraction/tests.rs` and verify it fails before the fix
- [x] 1.2 In `collect_imports` (`go-extractor/src/extraction/imports.rs`), skip a trailing `v<digits>` segment when deriving the default alias; verify with `cargo test -p go-extractor`
