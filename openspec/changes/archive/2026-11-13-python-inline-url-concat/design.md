## Context

See proposal.md - Why. `identify_target_uri` calls `clean_python_string(raw)`, which cuts everything before the first quote: `BASE + "/x"` -> `/x`. Pass 3's `generate_target_uris` splits the template on `+`. It strips quotes only from `"..."` parts, resolves names from the env, and handles f-strings when a part contains `{`. Other parts are kept as text.

## Goals / Non-Goals

**Goals:**
- Inline concatenation gives the same target as the variable form.

**Non-Goals:**
- Parenthesized concatenation, `urljoin`, `%` or `.format` targets.
- Any change to `clean_python_string` or its other callers.
- The empaia nondeterminism (see TODO.md).

## Decisions

### D1. Fix in `identify_target_uri`, not in `clean_python_string`
`clean_python_string` serves decorators, message edges and the parser, where cutting is either harmless or relied on. A local branch cannot affect them.
- Alternative: make `clean_python_string` keep text before the quote. Rejected: wider blast radius for one caller's problem.

### D2. Trigger
The branch applies when `raw` splits on `+` into two or more operands, the first is not a string literal, and each non-literal operand is a dotted name. D4 has already excluded `+` inside quotes, and a nested `+` leaves fragments that fail the name check. Everything else takes today's path, which keeps "other forms unchanged" true by construction.

### D3. Operand normalization
Rebuild the template joined with ` + `:
- Plain, `r` and `b` literals and brace-free f-strings, in any quote style -> `"content"`, the only quoted form pass 3 strips.
- f-strings containing `{`, in any prefix case or quote style -> `f"content"`; pass 3 resolves them, and it strips only a lowercase `f` prefix and single quote characters.
- Dotted names -> `{name}`. Pass 3 resolves a known value and leaves an unknown one as a hole, which matching aligns by suffix (`synthesizer/src/sdg/matching.rs`).
- Anything else (calls, subscripts, comments, `"/a" % x`) -> today's path, whose `/x` still matches by suffix.

### D4. Literal containing `+`
Pass 3 splits templates on every `+`, even inside quotes, so `BASE + "/a+b"` would break apart. Such a target takes today's path: the known wrong result stays, rather than a new malformed one. This check runs first, on any `+` inside a quoted operand.

### D4b. Pass 3 edits
`get_resolved_parts` treats a part that is exactly `{name}` as `name`, so constants holding raw f-strings resolve as in the variable form. The `Literal` arm now tests for a real f-string prefix (`f"`, `f'`): `s.starts_with('f')` turned `frontend:8000` into `rontend:8000`.

### D5. The `/` filter
Today a target with a literal and no `/` is dropped, as a dictionary lookup. No `/` in any literal operand -> today's path, which applies that filter. So `BASE + "x"` stays unreported, as it is today.

## Risks / Trade-offs

- [Unresolved operand: `client.get(svc_url + "/x")` gave `/x`, tagged `exact`; it now gives `{svc_url}/x`, tagged `suffix`] -> Accepted by the user: same polyglot edges (3 sites). Bare text `svc_url/x` was tried first and lost edge ts-travel-plan -> ts-train, since matching treats it as literal.
- [Inline and variable forms differ when an operand is unresolved: variable form gives bare `url`, since pass 3 `get_resolved_parts` ignores `Expr::Concat`] -> Accepted. Rendering `Concat` would change variable-form targets across the corpora, so it stays out of scope.
- [Empaia varies per run, so a union comparison cannot be exact] -> An AST scan lists the reachable call sites; task 3.1 uses both.
