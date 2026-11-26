# Proposal

## Why

The Python extractor aborts the whole run when a REST call target template contains a part that is a single `"` character (issue #56). The quoted-literal check treats a lone `"` as both opening and closing quote and slices out of bounds, so one odd call target takes down extraction for the entire project.

## What Changes

- A template part is treated as a quoted string literal only when it has both an opening and a closing quote (at least two characters).
- A lone `"` part no longer panics; it falls through to the normal (unresolved) handling and is kept as-is.
- Empty string literals (`""`) keep resolving to an empty string.

## Capabilities

### New Capabilities
- `python-rest-call-uri-resolution`: Resolving a Python REST call target template into concrete URIs without crashing on malformed or degenerate parts.

### Modified Capabilities

## Impact

- Code: `python-extractor/src/extraction/restcalls/evaluation/uri_generator.rs` (`get_resolved_parts`).
- No API, dependency, or output-format changes for templates that resolved before.
