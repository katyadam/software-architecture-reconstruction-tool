## Why

`path("api/", include("app.urls"))` is how Django projects split routes across apps. The `include()` entry is skipped, so routes in `app/urls.py` are reported without `api/`, and REST calls to the full path lose their exact match (issue #64).

## What Changes

- `path(prefix, include("a.b"))` and `re_path(regex, include("a.b"))` add `prefix` to every route in module `a.b`. This also covers the tuple form `include(("a.b", "app"))` and `namespace=`.
- Includes nest, and a module included twice yields one endpoint per prefix.
- The included module is found by walking up from the including file, so a same-named module in another service is not picked.
- Unknown modules (third-party apps), non-literal module arguments and `re_path` regexes with metacharacters add no prefix. Those routes keep today's URI.
- The neutral IR gains one fact, for a child router in another file. The resolver follows it and stays language-agnostic.
- `include()` entries still produce no endpoint.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `endpoint-route-prefixes`: Django `include()` now produces prefixes, and router composition can cross files through a module reference.

## Impact

- `models/src/route_facts.rs`: new fact variant.
- `extractor-runtime/src/pipeline/pass2/route_prefixes.rs`: edges whose parent is in another file.
- `python-extractor/src/extraction/route_facts/{django,mod}.rs`: new adapter.
- `docs/endpoint_restcall_matching.md`: known limits.
- Tests: adapter, resolver and `extractor-runtime/tests/pipeline/framework_endpoints.rs`.
- No file in train-ticket, train-ticket-polyglot or empaia contains `urlpatterns`, so corpus output must not change. Coverage comes from tests only.
