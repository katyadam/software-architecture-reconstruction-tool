# Design

## Context

Tree-sitter parsing is iterative, but the extractors walk the tree with many recursive functions spread over three crates. Measured in a debug build on a 2 MiB thread, the walks overflow at about 400 levels (Python), 460 (Java) and 765 (Go).

## Goals / Non-Goals

**Goals:** no process abort on deep input; a size cap on the extraction path.

**Non-Goals:** rewriting the walkers iteratively; impact-analysis parsers (`extractor-runtime/src/impact`, `java_analysis`), which are a separate path; Pass 3 symbolic evaluation depth (deep `Concat` chains built from many flat statements), which tree depth does not bound; memory used while buffering uploads before the size check; `agentic-sdg`-only code.

## Decisions

- **Gate the tree, not each walker.** One iterative `TreeCursor` depth check right after parsing bounds every recursive walk over the syntax tree. Converting dozens of walkers to iterative form was rejected as too large and easy to regress.
- **Limits 256 (Java, Python) and 512 (Go)**, about 1.5x below the measured crash depth. Real code is far shallower (CPython itself rejects ~200 nested parentheses). The test binary-searches the deepest accepted input of each crashing shape and proves it still extracts on a 2 MiB stack in a debug build.
- **Reject as an error.** The extractor returns `ExtractionError::Process`. The service already skipped Pass 1 errors (now logged at warn); the CLI propagated them and aborted, so it now skips the file with a warning like it does for unreadable files.
- **Size cap 4 MiB in `dispatch_syntactic`**, the single entry for Pass 1, for the source languages only (config files are read line by line). Generous: hand-written source files are far smaller.

## Risks / Trade-offs

- [A legitimate generated file deeper than 256 levels is skipped] → It is logged with the reason; the constant is easy to raise.
- [Frame sizes grow in future walkers] → The just-under-the-limit regression test fails first.
