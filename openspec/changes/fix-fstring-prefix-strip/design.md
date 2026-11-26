# Design

## Context

`resolve_fstring` is reached from two places in `get_resolved_parts`: a raw template part containing `{` (f-string source such as `f"a/{b}"`), and a variable's resolved `Expr::Literal` value that contains `{` or starts with `f`. Resolved values are already unquoted, but `resolve_fstring` calls `strip_prefix('f')` on every input it gets.

## Goals / Non-Goals

**Goals:**
- Strip `f` only when it is a real f-string prefix.

**Non-Goals:**
- Other string prefixes (`F"`, `rf"`, `fr"`), and the existing trimming of stray quotes from plain values.
- Changing which inputs reach `resolve_fstring`.

## Decisions

- Fix the check at the one place the `f` is stripped: strip it only if the text starts with `f"` or `f'`. This covers both entry paths with one change. Alternative: stop resolved values from reaching `resolve_fstring`; rejected because values with `{...}` placeholders still need placeholder resolution.

## Risks / Trade-offs

- [A resolved value that literally starts with `f"` would still lose its `f`] → Very unlikely in a URL; accepted.
