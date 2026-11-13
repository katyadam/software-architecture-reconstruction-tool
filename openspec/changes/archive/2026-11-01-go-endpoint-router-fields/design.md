## Context

See proposal.md - Why. Current state:

- `endpoints.rs::endpoint_from_call` runs the strategies in `endpoint_frameworks/` on every `call_expression`. The first `EndpointMatch {method, path, handler}` goes to `build_endpoint`, which sets `router_variable: None, enclosing_function: None` (`endpoints.rs:110-111`).
- Receiver node per strategy:
  - chi, ServeMux, web: `params.node.function.operand`.
  - gorilla: the operand of the inner `HandleFunc` call (`r` in `r.HandleFunc(..).Methods(..)`).
- chi and ServeMux accept a receiver only if it traces to a known constructor or package in the same file (`shared.rs::is_known_router_receiver`, `router_prefix`, `is_serve_mux_receiver`). Router parameters (`func f(r chi.Router)`) and closure parameters (chi `Route`) are not extracted today. gorilla does not check its receiver.
- Go callables: `name` is the bare identifier, `signature` is `func (s *Server) routes(r chi.Router) http.Handler` (`ir.rs::callable_signature`). Call statements store the signature as `enclosing_function_name`.
- Resolver (`route_prefixes.rs`) keys endpoint scope by `bare(enclosing_function)` and returns early when no file has facts. Go has no adapter, so Go endpoints pass through unchanged.

## Goals / Non-Goals

**Goals:**
- Fill both fields for every Go endpoint the current strategies produce.

**Non-Goals:**
- Accepting more receivers (parameters, closure params). That is backlog item 2/3; record there.
- Fixing `bare()` for Go signatures. That is item 2's problem (see Risks).

## Decisions

### D1. Router travels on `EndpointMatch`
Add `router: Option<String>` to `EndpointMatch`. Each strategy sets it from the receiver node it already inspects, through one shared helper `router_variable(receiver: Node, code, imports) -> Option<String>` in `shared.rs`:
- normalized receiver text (`normalize_whitespace(node_text)`), e.g. `r`, `s.router`;
- `None` when the leading identifier (text before the first `.`) equals an import's `module_alias` (`http`, `http.DefaultServeMux`, `web`).
- Alternative: re-derive the receiver in `endpoints.rs` from the call node. Rejected: gorilla's receiver is nested, and only the strategy knows which call it matched.

### D2. Enclosing function from the AST parent chain
In `endpoints.rs`, walk `node.parent()` up to the first `function_declaration` or `method_declaration` and take its `name` field text. Closures (`func_literal`) are passed over, so their registrations get the outer declaration. `None` at top level.
- Alternative: look up the callable by byte span. Rejected: needs span data that `collect_endpoints` does not have. The parent walk is three lines.
- It stores the bare name, as Python does (`decorator.rs:55`), so the resolver's `bare()` is a no-op here.

### D3. No resolver change
Go stays out of `facts_for`. Endpoint URIs cannot change, so the polyglot run is the regression check.

## Risks / Trade-offs

- [Resolver `bare()` on Go call-site signatures yields `func` (it splits at the first `(`), so Go caller scopes will not match endpoint scopes] -> not hit in this change (no Go facts). Item 2 must normalize Go signatures. Add a line to the backlog.
- [Mixed-language project: Python facts exist, so the resolver runs over Go endpoints too. With a router set, it may match a Python caller by function name] -> Caller lookup prefers the import graph and then the same file, and a Go file has no Python importers. Covered by the polyglot regression (Go + Python, 264 endpoints).
- [Go endpoints in `sdg.json` change (`router_variable` was `null`)] -> scoring reads only URI, method and connections, so nothing scored changes. The regression compares those fields.

## Migration Plan

None. Rollback = revert the commit.
