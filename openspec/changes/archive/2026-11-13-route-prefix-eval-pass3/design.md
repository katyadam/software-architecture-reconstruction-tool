## Context

See proposal.md - Why. Current flow:

- Pass 2 `build_project_ir` (`pass2/mod.rs`) runs `resolve_route_prefixes` before `identify_edges` and `post_process_endpoints`. The resolver walks Attach / Own facts, evaluates each prefix with `Resolver::eval` (literal, or one global constant in the same or import-resolved file), joins, and clones endpoints per prefix.
- Pass 3 `evaluate` / `evaluate_with_llm` (`pass3/mod.rs`) copy endpoints unchanged into `EvaluatedIR`. REST calls are evaluated per file with `env::build_constants_env` + `env::build_file_env` (project constants, external constants, per-file attrs, module consts).
- `ProjectIR` passes from pass 2 to pass 3 in-process in both callers (`cli/src/lib.rs`, `extractor-runtime/src/api/service.rs`). It is never serialized in between.

## Goals / Non-Goals

**Goals:**
- One evaluator for prefix values and REST call URLs.
- Pass 2 stays structural, pass 3 does all value evaluation.
- Same endpoint output as today whenever today's evaluator already resolved the prefix.

**Non-Goals:**
- No new route-fact adapters or idioms.
- No change to the walk: parameter following, callee resolution, depth cap and visited guard stay as they are.
- No running the full symbolic evaluator over the callable that registers the route. Only the prefix expression is evaluated, in the file env.
- f-string prefixes: they resolve only if the expression evaluator already supports them. No extra work here.

## Decisions

### D1. Split walk and evaluation at the pass boundary
Pass 2 returns prefix chains. Pass 3 evaluates and joins them.
- Alternative: move the whole resolver to pass 3. Rejected: the walk needs the import graph and call sites. That is structural linking, which belongs in pass 2, and it would mix two concerns into the evaluation pass.
- Alternative: keep pass 2 and give `Resolver::eval` the pass 3 env. Rejected: the env depends on `external_constants`, `per_file_attrs` and `per_file_module_consts`, which pass 2 does not receive. Threading them in would blur the pass contract.

### D2. Representation: chains on the endpoint
Add `Endpoint.route_prefixes: Vec<Vec<PrefixExpr>>` in `models`, where `PrefixExpr { file_path: String, expr: String }`.
- The outer `Vec` holds the distinct chains through which the endpoint is reachable. The inner `Vec` is ordered outermost first.
- `file_path` is the file where the expression is written. An Attach prefix lives in the mounting file, an Own prefix in the router's file. Constants must be looked up in that file's env, not the endpoint's file.
- Empty outer `Vec` means no prefix applies (today's "no facts" case).
- `#[serde(skip)]`: internal pipeline state, so the output shape is unchanged (spec: "Endpoint output shape is unchanged").
- Alternative: a side table in `ProjectIR` keyed by (file, endpoint index). Rejected: indices break when endpoints are dropped or cloned by `post_process_endpoints`.

### D3. Pass 2 still emits one endpoint, not clones
Pass 2 no longer clones endpoints. Each endpoint carries all its chains, and pass 3 clones once the values are known. Two chains that differ textually but resolve to the same string then collapse into one endpoint (the `BTreeSet` over resolved prefixes, as today).

`post_process_endpoints` (Python) runs after the resolver and clones `urlpatterns` endpoints per HTTP method. The `route_prefixes` field is copied with each clone, so this stays correct. It reads only `uri.is_empty()` and `router_variable`, which this change does not alter before pass 3.

### D4. Pass 3 prefix step
New `pass3/route_prefixes.rs`, called from the code shared by `evaluate` and `evaluate_with_llm`, replacing the plain endpoint copy:
1. Build `constants_env` once, and a file env per file referenced by any `PrefixExpr` (cached by path).
2. For each endpoint with chains: evaluate each expression to a string in its file's env; if any expression in a chain fails, the chain resolves to "" (keep the declared URI); otherwise join the chain parts with the existing `join` semantics.
3. Collect the resolved prefixes into a `BTreeSet`. Emit one clone per prefix with `uri = join(prefix, uri)` (an empty prefix keeps the URI).
- Expression evaluation: a small helper in `pass3/route_prefixes.rs`. Quoted literal (`"`, `'`, backtick) -> content. Otherwise exact env key lookup (dotted keys included), resolved only when the value is `Expr::Literal` and not a raw f-string. Anything else -> unresolved. `generate_uris` is not reused: it returns unknown names unchanged and Go keeps quotes, so residuals cannot be told from values. Concatenated prefixes (`ROOT + "/x"`) stay unresolved, as today.
- Go needs no extra source: its adapter already inlines same-file constants as quoted literals.
- `join` moves to pass 3, next to its only remaining caller.

### D5. File-local bindings beat project-wide constants in `build_file_env`
`collect_constants` is a flat, first-wins map keyed by name, and `build_file_env` adds module and imported constants with `or_insert`, so they never override it. Polyglot has `BASE_PATH` in both ts-consign-service and ts-travel-plan-service: with the unchanged env one service would get the other's prefix. `build_file_env` and `fill_env_with_imported_constants` now overwrite for module and imported constants. Attrs keep `or_insert`.
- This also changes REST call evaluation, deliberately: same latent bug, and one env keeps prefixes and URLs consistent.
- Alternative: file-local lookup for prefixes only. Rejected by the user: prefixes and URLs could disagree on a colliding name.

### D5b. Prefix evaluation details found during implementation
- External-only names keep priority over file-local bindings; overwrite applies only to names in project constants (`cli_override_wins`).
- Prefix env drops values that are still source text (`settings.base`, `os.getenv(...)`), since `collect_constants` stores raw right-hand sides. Go values are stored unquoted, so path-like values from Go files are accepted.
- `pass_module` runs a second round seeding imported constants, so `from svc.config import ROOT; BASE = ROOT` uses that service's `ROOT`. Bounded: two rounds, read from a snapshot.
- A name not bound in the file uses a project constant for prefixes only when all its declarations in the project share one value; otherwise unresolved. REST calls keep first-wins, which remains order-dependent on collisions.

### D6. Delete the pass 2 evaluator
Remove `Resolver::eval`, `unquote` and the `by_path` lookups used only for constants. The resolver tests that asserted evaluated URIs (`prefix_from_global_constant`, `prefix_from_imported_constant`, `unresolved_prefix_keeps_uri`) move to pass 3. Pass 2 tests assert the chains instead.

## Risks / Trade-offs

- [D5 changes REST call URLs where names collide] -> Diff connections and REST calls against the baseline; every change must be explained, and service-level scores must not drop.
- [Module-const harvest may include seeded project constants, not only the file's own bindings] -> A test with two same-named constants in different files covers the override.
- [Endpoints in `ProjectIR` now have a URI without prefixes until pass 3] -> Two readers remain: `post_process_endpoints` (emptiness only) and Go `is_route_registration` (`go-extractor/src/extraction/identify.rs:264`), which compares the raw route argument with `endpoint.uri`. The unprefixed URI is what it needs: a prefixed one could never match. Pipeline test covers a group route not becoming a REST call.
- [Env per referenced file held in memory during evaluation] -> Accepted. Built eagerly, only for files referenced by a `PrefixExpr`.
- [Wide router fan-out explodes chains] -> `MAX_CHAINS` = 256 per merge, deterministic truncation with a warning. Non-literal prefixes no longer collapse to one `None` during the walk, so this cap is required.

## Migration Plan

Single PR on top of `feat/endpoint-route-prefixes`. No data migration: the new field is not serialized. Rollback = revert.
