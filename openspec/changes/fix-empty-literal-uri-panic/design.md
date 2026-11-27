# Design

## Context

`get_resolved_parts` splits the template on `+` and strips quotes from any part that starts and ends with `"` via `part[1..part.len() - 1]`. For a one-character part `"`, both checks pass and the slice is `[1..0]`, which panics.

## Goals / Non-Goals

**Goals:**
- Remove the panic with the smallest possible change.

**Non-Goals:**
- Making the `+` split quote-aware (a literal such as `"a+b"` is still split in two). That is a separate behavior change.

## Decisions

- Add `part.len() >= 2` to the quoted-literal condition (the fix suggested in #56). Alternative: `strip_prefix('"').and_then(|p| p.strip_suffix('"'))`. It works too, but it rewrites more of the line for the same result.

## Risks / Trade-offs

- [A lone `"` now reaches the env lookup / fallback branch and is emitted verbatim] → Acceptable: the URI is already garbled by the naive split, and keeping it beats crashing.
