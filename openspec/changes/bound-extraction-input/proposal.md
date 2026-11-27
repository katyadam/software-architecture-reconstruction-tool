# Proposal

## Why

Extractors read untrusted uploaded source (#70). On `main`, deeply nested source (a few hundred levels of parentheses, `+` chains, or nested Python `if` blocks) overflows the 2 MiB thread stack in the recursive AST walks and aborts the whole extractor-runtime process. There is also no size limit on files entering extraction.

## What Changes

- Files whose syntax tree is deeper than 256 levels (Java, Python) or 512 levels (Go) are rejected by the syntactic extractors with an extraction error, instead of crashing.
- The extractor-runtime service and the CLI log such a file and continue with the next one (the CLI used to abort the whole run on any extraction error).
- Java, Python and Go files larger than 4 MiB are skipped by the Pass 1 dispatcher with a warning.
- Not applicable to `main` (exists only on `agentic-sdg`): the route-prefix traversal bound and the quadratic `scoped_router` from #70.

## Capabilities

### New Capabilities
- `extraction-input-limits`: limits on source input (nesting depth, file size) that keep syntactic extraction from crashing or exhausting resources.

### Modified Capabilities

## Impact

- `statix/src/util.rs`: depth limit and an iterative depth check.
- `go-extractor`, `python-extractor`, `java-extractor`: `extract_syntactic` (and Java `extract_syntactic_callables`) reject over-deep trees.
- `cli/src/lib.rs`, `extractor-runtime/src/api/service.rs`: skip a failing file with a warning.
- `extractor-runtime/src/pipeline/pass1.rs`: size limit in `dispatch_syntactic`.
- Behavior: such files produce no records; all other files are unaffected.
