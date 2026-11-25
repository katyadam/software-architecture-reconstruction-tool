# Proposal

## Why

Go modules at major version 2+ put a `/vN` suffix on the import path (`github.com/go-chi/chi/v5`), but the package is still named `chi`. The Go extractor took the last path segment as the import's alias, so it recorded `v5`. Router detection matches receivers by import alias, so code like `router := chi.NewMux(); router.Get("/items", h)` lost its endpoint (issue #68). The same happens for `echo/v4`, `fiber/v2` and similar.

## What Changes

- Unaliased Go imports whose path ends in a major-version segment (`v2` or higher) take the segment before it as their alias.
- Explicit aliases (`chi "github.com/go-chi/chi/v5"`) and all other paths keep their current behavior.
- Not applicable to `main`: the `is_router_parameter` workaround named in the issue exists only on the `agentic-sdg` branch, so there is nothing to remove here.

## Capabilities

### New Capabilities
- `go-import-extraction`: how the Go extractor derives the alias of each import it records.

### Modified Capabilities

## Impact

- `go-extractor/src/extraction/imports.rs` (`collect_imports`).
- Anything that matches on Go `module_alias` (router/serve-mux detection, `web.go` detection, REST call identification) now sees the real package name for versioned modules.
