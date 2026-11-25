# Design

## Context

`EndpointsExtractor::extract` runs one tree-sitter query that matches methods and class annotations. Class annotation matches overwrite a single `shared_uri`, which is prepended to every endpoint at the end of the file. `extract_uri` uses `split_once('=')`, so it cannot handle more than one named argument.

## Goals / Non-Goals

**Goals:** B1–B4 and B6 from issue #71 with minimal code.

**Non-Goals:** constant resolution (B5), subresource locators (B7), method-level path arrays, the shared resolver from `agentic-sdg`.

## Decisions

- **Enclosing type via the syntax tree, not query matches.** For each matched method, walk `parent()` to the nearest `class_declaration` / `interface_declaration` / `enum_declaration` / `record_declaration` and read its `modifiers` annotations. This gives the per-class prefix (B2), covers interfaces (Feign, API interfaces), and lets us drop the `class_annotation` query pattern. Alternative: capture class + method in one query pattern. Rejected because nested bodies make the pattern brittle.
- **Annotation name match by exact simple name.** `@RequestMapping`/`@Path` are matched by their name (the text before `(`, last `.` segment), so `@PathVariable` and similar never match. `@FeignClient` is matched the same way, so fully-qualified `@org...FeignClient` also counts.
- **Small top-level argument splitter.** Split the argument list on commas outside quotes and braces, then split each argument on its first top-level `=`. An unnamed argument is `value`. Array values `{...}` are split the same way. Only string literals become paths. Non-literals (constants) give no path, as before.
- **Method verb from `method =`.** Take the last `.` segment of the first item and map GET/POST/PUT/DELETE/PATCH/HEAD/OPTIONS.

## Risks / Trade-offs

- [A class without a route annotation now gets no prefix, where before a stray annotation could leak in] → that is the intended fix. Existing corpora tests guard regressions.
- [Escaped quotes in literals] → the splitter skips `\"` inside strings.
