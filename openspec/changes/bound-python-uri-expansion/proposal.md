# Proposal

## Why

The Python extractor expands a REST call target into every combination of its parts' possible values with no limit (issue #57). A target that concatenates many multi-valued constants grows exponentially (20 two-valued parts already give over a million URIs), which can exhaust memory and abort the whole extraction run.

## What Changes

- URI resolution for a single REST call target returns at most a fixed number of URIs (256).
- Past the limit the choice is deterministic: the first URIs in combination order (each part's values in their resolved order) are kept, the rest are dropped.
- Targets that resolve to fewer URIs than the limit produce exactly the same output as before.

## Capabilities

### New Capabilities
- `python-rest-call-uri-resolution`: Resolving a Python REST call target template into concrete URIs. (Introduced by the not-yet-archived change `fix-empty-literal-uri-panic`; this change adds a requirement to the same capability.)

### Modified Capabilities

## Impact

- Code: `python-extractor/src/extraction/restcalls/evaluation/uri_generator.rs` (`generate_target_uris`).
- Output: targets with more than 256 combinations now yield 256 URIs instead of all of them (or a crash).
