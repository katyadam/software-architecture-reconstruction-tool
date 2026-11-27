# Proposal

## Why

The Java endpoint extractor builds class-level route prefixes wrongly (issue #71). Every class annotation is read as a path, the last one wins, and that prefix is added to every endpoint in the file. Common Spring/JAX-RS code (`@CrossOrigin`, `@Api`, `@Produces`, several classes in one file, `method = RequestMethod.POST`, Feign clients) therefore yields wrong URIs, wrong verbs, or endpoints that do not exist.

## What Changes

- Class prefix comes only from `@RequestMapping` / `@Path` on the method's own enclosing class or interface (B1, B2).
- Annotation arguments are parsed per argument, so `value=` / `path=` work next to other arguments and `method = RequestMethod.X` sets the HTTP method (B3).
- Methods inside a type annotated `@FeignClient` are not endpoints (B4).
- A class prefix array `{"/a", "/b"}` yields one endpoint per prefix (B6).
- Not in scope (not applicable to `main`): B5 constant resolution and B7 JAX-RS subresource locators. Both need the shared route-prefix resolver, which exists only on `agentic-sdg`.

## Capabilities

### New Capabilities
- `java-endpoint-routes`: how the Java extractor derives an endpoint's HTTP method and URI from Spring / JAX-RS annotations.

### Modified Capabilities

## Impact

- `java-extractor/src/extraction/endpoints/` (extractor + annotation parsing), `ENDPOINTS_QUERY` in `queries.rs`.
- New regression tests in `java-extractor/tests/java/endpoints.rs`. No API or model changes.
