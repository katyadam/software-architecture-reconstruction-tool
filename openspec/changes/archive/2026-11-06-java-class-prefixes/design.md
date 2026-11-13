## Context

See proposal.md - Why, and issue #71. Today:

- The Java endpoints query (`queries.rs`) matches annotated methods and every class annotation.
- `extract` sets `shared_uri` from each class annotation it meets, so the last one wins. After the loop it concatenates that value onto every endpoint in the file.
- Java endpoints have no `router_variable`.

The shared pipeline already has what this change needs:

- **Pass 2** (`pass2/route_prefixes.rs`) writes `route_prefixes` chains of `PrefixExpr { file_path, expr }` for every endpoint. An endpoint with no router gets `vec![]`, which clears any chain already present. When no file has facts, pass 2 returns early.
- **Pass 3** (`pass3/route_prefixes.rs::expand`) evaluates each chain in its file's env and emits one endpoint per distinct prefix, joined with `/` normalization. A chain with an unresolved part evaluates to `""`, and the endpoint keeps its declared URI.
- **Java constants:** they are stored in `assignments` with scope `Class` or `Global`. Pass 2 collects all-uppercase names into `project_ir.constants` by bare name. The prefix env uses a constant only when its declared value is unique project-wide.

## Goals / Non-Goals

**Goals:**
- Class prefix per enclosing type, through the shared pass-3 evaluation.
- No change to the URI sets of the three corpora.

**Non-Goals:**
- B7, subresource locators. They need cross-file type lookup.
- Constants in method-level paths.
- Spring `server.servlet.context-path` (#66).
- Lowercase constant names, which pass 2 does not collect.
- `@RequestMapping` with no `method=` matches every HTTP method in Spring. It stays `GET`.

## Decisions

### D1. The extractor writes prefix chains directly; no route-fact adapter
The Java extractor puts one single-element chain per class prefix into `endpoint.route_prefixes`, as `PrefixExpr { file_path: <this file>, expr }`.

- A literal keeps its quotes: `"\"/api\""`.
- A constant becomes its bare name (D3).
- Pass 3 then evaluates the chains and expands them, as it does for Python and Go.
- `endpoint.uri` holds only the method path.

Alternative considered: a `facts_for` arm for Java, with `Own { router: <ClassName> }` facts and `router_variable = <ClassName>`. Rejected for two reasons:

- The adapter reads a `TypedFileRecord`, which holds no class annotations. The extractor would have to carry them anyway.
- A Java class prefix is static. There is no router graph to walk.

The chain format is the neutral IR. Only the graph walk is skipped.

### D2. Pass 2 keeps chains on router-less endpoints
In `resolve_with_facts`, an endpoint with no `router_variable` keeps its `route_prefixes`. This replaces `vec![]`, and it is a one-line guard. It matters only for mixed-language projects, because a Java-only project has no facts and pass 2 returns before touching anything.

### D3. Constant prefix expressions
A non-literal class prefix of the form `Ident` or `A.B.IDENT` is emitted as its last segment, `IDENT`, because the env is keyed by bare constant name. Anything else, such as a call or a concatenation, is emitted as written. It won't resolve, so it is dropped by the existing rule.

Alternative considered: keep the qualified name and teach pass 3 to strip it. Rejected, because it is a Java-specific rule inside language-agnostic code.

### D4. Enclosing type by tree walk
For each method match, walk the parents to the nearest `class_declaration`, `interface_declaration`, `enum_declaration` or `record_declaration`. Use `get_enclosing_node_by_kind`, or a loop over the four kinds. Then read that node's `modifiers` with `get_annotations_from_modifiers`.

- Only annotations that start with `@RequestMapping` or `@Path` count. Match `@Path(` and `@Path` exactly, so `@PathVariable` never matches.
- The `class_annotation` capture and `shared_uri` go away.

### D5. Arrays and `method=`
- `extract_uri` returns only the first array element. A new sibling, `extract_uris`, returns every literal element. A non-literal value comes back as a single raw element.
- `method = RequestMethod.X` or `{RequestMethod.X, ...}` is parsed from the same top-level arguments by taking the last `.`-segment of each element.
- The extractor emits one `Endpoint` clone per (method path × HTTP method). Class prefixes stay as chains, and pass 3 expands them.

### D6. Feign
If the enclosing type is an `interface_declaration` whose annotations include one starting with `@FeignClient`, skip the method.

## Risks / Trade-offs

- **The extractor's output changes shape.** Its `uri` no longer contains the class prefix, so consumers that read extractor output without pass 3 see only the method path. Mitigation: in this repo the only consumer is the pipeline, which runs pass 3. `java-extractor/tests/java/endpoints.rs` expectations get updated, and the end-to-end tests check the joined URI.
- **An unresolved constant prefix is now dropped instead of glued on as raw text.** Mitigation: the raw text never matched as a literal anyway, and `/x` aligns by suffix. This matches the Python and Go behaviour, per the "Unresolved prefixes keep today's URI" requirement.
- **A constant name declared with different values in two services fails the uniqueness check, so the prefix is dropped.** This is accepted and is the same rule as Python and Go.
- **Corpora unchanged.**
  - All three use one literal `@RequestMapping` or `@Path` per controller.
  - Pass 3 `join` gives the same result as concatenation for `"/api/v1/x"` + `"/y"`.
  - Mitigation: diff the endpoint URI sets before and after.
  - Caveat: if a corpus prefix ends in `/`, or a method path lacks a leading `/`, the old URI differs (`/api//x`, `/apix`). The diff shows this; the new result is the correct one.
