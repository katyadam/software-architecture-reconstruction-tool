# Proposal

## Why

When the Python extractor resolves a REST call target, it drops the first letter of any resolved value that starts with `f` (issue #58). The code meant to remove the `f` prefix of an f-string (`f"..."`) also runs on plain values whose quotes are already gone. So `HOST = "frontend/{x}"` followed by `HOST + "/y"` resolves to `rontend/{x}/y`.

## What Changes

- The `f` is removed from a target part only when it is a real f-string prefix, meaning a quote (`"` or `'`) comes right after it.
- Values that just happen to start with `f` (such as `frontend/...` or `files`) keep their first letter.
- Real f-string targets (`f"frontend/{id}"`) resolve as before.

## Capabilities

### New Capabilities
- `python-rest-call-uri-resolution`: Resolving a Python REST call target template into concrete URIs. (Introduced by the not-yet-archived change `fix-empty-literal-uri-panic`; this change adds a requirement to the same capability.)

### Modified Capabilities

## Impact

- Code: `python-extractor/src/extraction/restcalls/evaluation/uri_generator.rs` (`resolve_fstring`).
- Output: URIs built from values starting with `f` now keep that letter.
