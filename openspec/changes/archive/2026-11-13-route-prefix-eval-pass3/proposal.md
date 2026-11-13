## Why

Route prefix resolution (`extractor-runtime/src/pipeline/pass2/route_prefixes.rs`) evaluates prefix expressions with its own small evaluator (`Resolver::eval` + `unquote`): a quoted literal or one level of constant. Pass 3 already evaluates REST call URLs with a much richer symbolic environment (project constants, CLI `external_constants`, class attribute defaults, module constants). As a result, prefixes from settings, chained constants or external constants stay unresolved, and endpoint URIs and REST call URLs are evaluated by two different rules.

## What Changes

- Pass 2 keeps the structural walk (Attach / Own facts, following router parameters through call sites via the import graph). It no longer evaluates or joins prefixes. Instead it records, per endpoint, the set of unevaluated prefix chains (raw expression text, outermost first, plus the file each expression belongs to).
- Pass 3 evaluates each prefix expression in the same file environment used for REST calls, joins the chain with the endpoint URI, and emits one endpoint per distinct resolved prefix.
- A chain with any unresolvable expression keeps the original URI (unchanged behavior).
- `Resolver::eval` and `unquote` are deleted from pass 2.
- `models::Endpoint` gets a new field holding the unevaluated prefix chains. It is internal pipeline state: not serialized into extractor output, so the JSON shape does not change.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `endpoint-route-prefixes`: prefix values are resolved with the same constant environment as REST call URLs, including external constants (e.g. `settings.base`) and constants defined in terms of other constants. The "Unresolved prefixes keep today's URI" requirement is narrowed: `settings.base` now resolves when an external constant supplies it.

## Impact

- `models/src/endpoints.rs`: new `Endpoint` field (serde skip).
- `extractor-runtime/src/pipeline/pass2/route_prefixes.rs`: walk returns expression chains, no evaluation, no cloning.
- `extractor-runtime/src/pipeline/pass3/`: new prefix evaluation step reusing `env::build_constants_env` / `build_file_env`; called from both `evaluate` and `evaluate_with_llm`.
- Python `post_process` (pass 2) is unaffected: it reads only `uri.is_empty()` and `router_variable`, neither of which the prefix join changes for routed endpoints.
- Regression: empaia endpoint set must stay identical without external constants; train-ticket (Java) and train-ticket-polyglot (264 endpoints, 89 connections) unchanged.
