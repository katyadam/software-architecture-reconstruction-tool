# Design

## Context

`CallsExtractor` already has the tree-sitter `argument_list` node for every call but only uses its text. tree-sitter-python exposes each top-level argument as a named child: `keyword_argument` (fields `name`, `value`), `comment`, or any expression (positional, including `*args` / `**kwargs`).

## Goals / Non-Goals

**Goals:** correct argument boundaries; `url=` keyword as REST target.

**Non-Goals:** comments *inside* an argument expression (issue #60); changing how targets are evaluated or concatenated.

## Decisions

- **Walk the node's named children instead of a smarter text splitter.** The parser already resolved strings, brackets and nesting; re-implementing that in a splitter is more code and still fragile. Argument values stay the source text of the value node, so downstream consumers see the same strings as before for simple calls.
- **Target = `url` keyword, else the first argument.** `url` is the parameter name in `requests` and `httpx`. Previously the first argument was used even when it was a keyword like `timeout=5`. Falling back to the first argument (not the first positional one) keeps wrappers like `api.get(path="/x")` detected as before.

## Risks / Trade-offs

- [Keyword values are now trimmed (`json = data` -> `json`/`data`)] -> only removes stray whitespace that previously leaked into names/values.

## Notes

- Resolving the base of an *inline* concatenation (`requests.get(BASE + "/x")` -> `http://svc/x`) exists only on the `agentic-sdg` branch (`concat_target`), not on `main`. On `main` an inline target keeps only its literal path. This change makes the argument correct so that code gets the whole expression once merged; tests assert on the path suffix so they hold either way.
