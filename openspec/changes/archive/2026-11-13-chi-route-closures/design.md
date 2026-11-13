## Context

See proposal.md - Why. Current state:

- `RouteFact::Attach` and the resolver key routers by `(file, scope, variable)`. A closure has no scope of its own: endpoints and call statements inside it report the enclosing named function. So an inner `r` and an outer `r` share one key.
- `is_router_parameter` rejects `func_literal` parameters. `receiver_origin` is scope-blind, so a shadowing `r` passes through the outer `r := chi.NewRouter()`, which is why `/x` appears today.
- Go call statements include calls nested in closures (`walk_named` visits all descendants) and carry `call_span`. Arguments keep raw text, so `func(r chi.Router) {...}` is readable from a record.
- `collect_callable_ir` runs before `collect_endpoints`, so endpoint identification can read call statements.
- `join` handles the rest: `join("/p", "/")` is `/p`, and `""` is skipped.

## Goals / Non-Goals

**Goals:**
- Closure routers reach the resolver as ordinary facts, with one definition of closure scoping.

**Non-Goals (route stays without the closure's prefix):**
- A helper called inside a closure: `r.Route("/p", func(r chi.Router) { users(r) })`. The call site's argument `r` resolves to the outer variable.
- A named function as the closure: `r.Route("/p", users)`.
- `x := r.Group("/y")` assigned inside a closure. Assignments have no span, so `r` means the outer variable.
- `r.With(mw).Get(...)`, not extracted today. `r.With(mw).Route("/p", closure)` gets `/p` but loses outer prefixes. So does the second call of a chained `r.Route(..).Route(..)`.
- Scoping for other languages. Only Go has these closures.

## Decisions

### D1. Each closure router gets a synthetic variable
The inner router is named `<param>@<call end byte>`, for example `r@412`, where the byte is the end of the `.Route` or `.Group` call. The end, because chained `r.Route(..).Route(..)` calls share a start, which gave `/a/x`, `/a/y`, `/b/x`, `/b/y` in place of `/a/x`, `/b/y`. The endpoint's `router_variable` uses it, and the fact is `Attach { scope, parent, child: "r@412", prefix }`.
- Resolver lookups, parameter following and pass 3 need no change, apart from a depth warning: `r@412` is not a parameter, so it follows `Attach` to `parent`. Nothing outside the extractors reads `router_variable` except the resolver and Python post-processing.
- Alternative: new `Endpoint` field holding closure prefixes, joined by the resolver. Rejected: model and resolver change, plus a second path for prefixes next to facts.
- Alternative: join the prefix in the extractor. Rejected: `go-route-fact-adapter` D4 removed that, and pass 3 owns evaluation.
- Alternative: a closure as a resolver scope. Rejected: `enclosing_function_name` is parsed as a signature by `identify.rs`.

### D2. One helper reads closures from call statements
`closure_routers(calls, imports)` lists each call whose name ends in `.Route` with two or more arguments and a func literal second, or `.Group` with a func literal first. It parses `func(<name> <type>)`, one parameter only, and checks the type with `ROUTER_PARAMETER_TYPES` and `imports_package`. Each entry holds call span, parameter, receiver and prefix text.
`scoped_router(closures, position, name)` returns the innermost entry, by smallest end, with `start < position < end` and that parameter name, as `name@end`, else `name`. The strict `start <` excludes a call's own receiver chain, which shares its start byte. A position inside an earlier closure of a chain is excluded too: it lies in the later call's span, but not in its closure.
- Alternative: an AST walk for endpoints plus a span walk for facts. Rejected: two definitions of one scoping rule must agree exactly.
- Fiber's `app.Route("/p", func(r fiber.Router) {...})` works without extra code, since the table lists it.

### D3. Adapter
`route_facts` adds one `Attach` per closure: parent = `scoped_router` of the receiver, prefix through existing `prefix_text`, `"\"\""` for `.Group`. `prefix_text` returning `None` drops the fact, so the route stays as declared. The `Mount` fact's parent also goes through `scoped_router`. Fact scope is the enclosing function, as for `Mount`.

### D4. Endpoint identification
`ExtractParams` gains the closure list, built once per file in `collect_endpoints`. `ChiStrategy` looks up the receiver first: a closure hit accepts the call and sets `router` to the synthetic name, before `receiver_origin` can pick up the outer variable. Other cases take today's path.
- A hit replaces `is_router_parameter` for closures, whose `func_literal` rejection stays. So closures passed to other calls stay rejected, unless a same-named outer router matches, as today.

## Risks / Trade-offs

- [`r.Route` closure holding an inner literal that redeclares `r` with a non-router type, then calling `r.Get("/x", y)`] -> False endpoint. Same ceiling as the existing nested-block shadowing note; check local bindings if it shows up.
- [Closure with several parameters, a comment in its parameter list, or a parenthesized form] -> Not recognized, today's path. chi and Fiber signatures take one.
- [`r@412` changes when the file changes] -> Harmless: only the resolver reads it, and pass 3 reads the resulting `route_prefixes`.
- [Each closure takes two `MAX_DEPTH` slots: 7 nested closures resolve, 8 or more lose every prefix] -> The route keeps its declared URI and the resolver now warns. No resolver lookup changes.
- [`scoped_router` scans all closures per lookup: 1.3 MB of sibling closures took 8.7s against 0.65s] -> Accepted, marked `ponytail:`. A crafted file already costs far more in `prefixes`, whose traversal is exponential; that predates this change.
- [Named function parameter used inside a closure on another name: `func routes(r chi.Router) { r.Route("/a", func(a chi.Router) { r.Get("/x", h) }) }`] -> No endpoint, as before.
- [Common helper and named-function forms stay out of scope] -> Their prefix stays missing, as today. Add when a corpus needs them; no local corpus uses chi.
- [Design is not prototyped] -> Task 1.1 writes failing tests first, on real extraction.
