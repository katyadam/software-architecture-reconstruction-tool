# Spec Delta

## Purpose

Bound the resources syntactic extraction spends on a single untrusted source file, so a crafted file is skipped instead of crashing the extractor.

## ADDED Requirements

### Requirement: Over-deep source is rejected without crashing
Syntactic extraction of Go, Python and Java source SHALL fail with an extraction error, and SHALL NOT abort the process, when the file's syntax tree nests deeper than 256 levels for Java and Python, or 512 levels for Go. Files at or below that depth SHALL be extracted as before. Callers SHALL skip the rejected file with a warning and continue with the remaining files.

#### Scenario: Deeply nested expression
- **WHEN** a Go, Python or Java file containing an expression nested 1000 levels deep is extracted on a thread with the default 2 MiB stack
- **THEN** extraction returns an error and the process keeps running

#### Scenario: Nesting just under the limit
- **WHEN** a file whose syntax tree is exactly at the depth limit is extracted on a thread with the default 2 MiB stack
- **THEN** extraction succeeds and returns a file record

### Requirement: Oversized files are skipped
The extraction pipeline SHALL skip, with a warning, any Java, Python or Go file larger than 4 MiB and produce no record for it.

#### Scenario: File over the size limit
- **WHEN** a `.py` file of more than 4 MiB is dispatched for syntactic extraction
- **THEN** no record is produced and no error is returned
