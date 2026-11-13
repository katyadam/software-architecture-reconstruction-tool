## Why

The route-prefix resolver keys every endpoint by its router variable and the function that registers it. Go endpoints set neither field, so no Go route-fact adapter (backlog item 2) or chi `Route` closure support (item 3) can attach prefixes to them. This change fills those two fields first. No URI changes yet.

## What Changes

- Go endpoints record `router_variable`: the receiver the route is registered on (`r` in `r.Get(...)`, `mux` in `mux.HandleFunc(...)`, `r` in `r.HandleFunc(...).Methods(...)`). It stays unset when the receiver is an imported package (`http.HandleFunc`, hoisie `web.Get`).
- Go endpoints record `enclosing_function`: the bare name of the function or method declaration that holds the registration (`routes`, not `func (s *Server) routes(...)`). Registrations inside a closure get the outer declaration's name.
- Endpoint URIs, methods, handlers and counts stay the same. Go has no route-fact adapter yet, so the resolver leaves Go endpoints as they are.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `endpoint-route-prefixes`: adds a requirement that endpoints from supported route-registration idioms carry their router and enclosing function, with Go scenarios.

## Impact

- `go-extractor/src/extraction/endpoint_frameworks/{mod,chi,gorilla,serve_mux,web}.rs`: `EndpointMatch` carries the router.
- `go-extractor/src/extraction/endpoints.rs`: `build_endpoint` fills both fields.
- `go-extractor/src/extraction/tests.rs`: new assertions.
- Go endpoints in extraction JSON and `sdg.json` gain `router_variable` and `enclosing_function` values. Endpoint URIs, methods and counts, and SDG connections, do not change.
- No other crate changes.
