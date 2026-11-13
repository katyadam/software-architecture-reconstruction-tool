## Why

`r.Route("/p", func(r chi.Router) { r.Get("/x", h) })` is the usual way to nest chi routes. It reports `/x` when the closure shadows an outer `r := chi.NewRouter()`, and nothing at all otherwise. Either way the service's API surface is wrong and inter-service edges to `/p/x` are lost (issue #63).

## What Changes

- Routes on the router parameter of a closure passed to `.Route(prefix, closure)` or `.Group(closure)` are extracted, whatever the parameter is called.
- `.Route` closures add their prefix. `.Group` closures add none, but still tie inner routes to the outer router. Nesting works up to the resolver depth limit, and the outer router's own prefix still applies.
- `.Mount` inside such a closure attaches to the closure's router, not to a same-named outer variable.
- Closures passed to any other call stay unextracted.
- No data model change and no resolver lookup change: closures become ordinary `Attach` facts. The resolver gains a warning when nesting passes its depth limit.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `endpoint-route-prefixes`: Go closure routers now produce prefixes. The rule that closure parameters are never extracted narrows to closures not passed to `.Route` or `.Group`.

## Impact

- `go-extractor/src/extraction/route_facts/mod.rs`, `endpoint_frameworks/{shared,chi,mod}.rs`, `endpoints.rs`, `mod.rs`; `extractor-runtime/src/pipeline/pass2/route_prefixes.rs` (warning only).
- Tests: `go-extractor/src/extraction/tests.rs`, `extractor-runtime/tests/pipeline/framework_endpoints.rs`.
- No local corpus imports chi, so corpus output must not change. Coverage comes from tests only.
