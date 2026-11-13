## Why

The Java extractor reads the class-level prefix from whichever class annotation comes last, and glues it onto every endpoint in the file. Common Spring and JAX-RS code therefore gets wrong URIs: `@Path("/users") @Produces(MediaType.APPLICATION_JSON)` yields `MediaType.APPLICATION_JSON/{id}`. Issue #71 lists the cases; this change fixes B1-B6.

## What Changes

- **B1:** only `@RequestMapping` and `@Path` on a type give a class prefix. Other annotations (`@Produces`, `@CrossOrigin`, `@Api`, ...) are ignored.
- **B2:** each method takes the prefix of its own nearest enclosing type, not the file's last one.
- **B3:** `@RequestMapping(method = RequestMethod.X)` gives method `X`, and a method array gives one endpoint per method. Without `method=`, the method stays `GET`, as today.
- **B4:** methods of an interface annotated `@FeignClient` are not endpoints. Other interfaces keep their endpoints and now also get their own `@RequestMapping` prefix.
- **B5:** a class prefix that is a constant (`@RequestMapping(Paths.BASE)`) resolves through pass 3, the same way Python and Go prefixes do. An unresolved prefix is dropped and the method path is kept. Today the raw text `Paths.BASE` is glued onto the URI.
- **B6:**
  - A class prefix array (`@RequestMapping({"/a", "/b"})`) gives one endpoint per prefix.
  - A method path array gives one endpoint per path.
- Class prefixes are joined with `/` normalization, not plain concatenation.
- Out of scope, left on #71:
  - **B7:** JAX-RS subresource locators.
  - Constants in method-level paths (`@GetMapping(Paths.ITEM)`). No corpus uses them, and they keep today's raw text.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `endpoint-route-prefixes`: Java class-level prefixes go through the shared prefix evaluation. Endpoints that already carry prefix chains, and have no router, keep those chains through pass 2.

## Impact

- `java-extractor/src/extraction/endpoints/{extractor,annotations}.rs` and `queries.rs`: per-type prefixes, `method=`, Feign skip, array expansion.
- `extractor-runtime/src/pipeline/pass2/route_prefixes.rs`: endpoints without a router keep their chains.
- Tests:
  - `java-extractor/tests/java/endpoints.rs`: expected values change, because the URI now holds only the method path and the prefix sits in `route_prefixes`.
  - New end-to-end cases in `extractor-runtime/tests/pipeline/framework_endpoints.rs`.
- `docs/endpoint_restcall_matching.md`: known limits.
- train-ticket, train-ticket-polyglot and empaia use no case in #71, so their endpoint URI sets must not change.
