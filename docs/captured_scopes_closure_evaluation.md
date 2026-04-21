# Captured Scopes — Closure REST Call Evaluation

## Problem

Python microservices commonly define REST calls inside nested functions (closures):

```python
def add_routes_classes(app):
    base_url = settings.as_url.rstrip("/")  # outer-scope variable

    @app.get("/classes", ...)
    async def _(...):
        return await http_client.get(f"{base_url}/v3/classes")  # uses base_url
```

When the inner callable is evaluated in isolation, `base_url` is not in its env, so the target URI collapses to the unresolved placeholder `{base_url}/v3/classes`.

## Solution: `captured_scopes`

`captured_scopes` is a `HashMap<String, Env>` built once per file that maps each inner (nested) function's body hash to the subset of the outer function's evaluated env that the inner function can access.

## Building `captured_scopes`

`build_captured_scopes` (in `pass3/restcalls.rs`) iterates over all callables in a file. For each outer callable that has `nested` refs (inner functions declared in its body):

1. Symbolically evaluate the outer callable once with the constants env.
2. From the resulting `final_env`, filter down to variables listed in `nested_ref.captured` — the set of variable names declared in the outer function's scope (computed at parse time by `collect_nested_refs` from `declared_vars`).
3. Drop any variables that resolved to `Expr::Empty` (unresolvable).
4. Store the survivors as an `Env` entry keyed by `nested_ref.hash` — a SHA-256 of the inner function's full source text.

## Using `captured_scopes` per REST call

In `evaluate_single_restcall`, before symbolic evaluation:

1. Look up `captured_scopes` by `restcall.function_hash`.
2. If found, merge the captured variables into `eval_env` alongside the constants env (`or_insert_with` so constants take precedence).
3. Run `symbolic_evaluation_with_env` seeded with this combined env.

Variables like `base_url` — assigned in the outer scope, referenced in the inner callable's REST call URI — are now present during evaluation, allowing the f-string to resolve to a concrete URI.

## Why hash instead of mangled name

Two sibling inner functions (e.g. `async def _` inside two different `add_routes_*` outers) produce identical mangled callable keys but have different function bodies. Keying `captured_scopes` by body hash ensures each inner function gets its own captured env entry — a mangled-name key would cause the second sibling to silently overwrite the first.

## Data flow summary

```
parse_python_function
  -> declared_vars (outer scope)
  -> collect_nested_refs(outer_body, declared_vars)
       -> NestedRef { key, hash, captured: outer_vars }

build_captured_scopes (per file, before rest call loop)
  for each outer callable with nested refs:
    eval outer with constants_env -> final_env
    for each NestedRef:
      captured_scopes[nested_ref.hash] = final_env filtered to nested_ref.captured

evaluate_single_restcall (per rest call)
  eval_env = constants_env
           + captured_scopes[restcall.function_hash]  (if nested)
  symbolic_evaluation_with_env(callables, mangled, eval_env)
  -> resolved target URI
```
