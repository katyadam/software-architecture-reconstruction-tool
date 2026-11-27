# Spec Delta

## Purpose

Defines how the Go extractor names each import it records, so later passes can match code references like `chi.NewMux()` back to the imported module.

## ADDED Requirements

### Requirement: Unaliased import alias is the Go package name
For an import without an explicit name, the recorded alias SHALL be the last path segment, except when that segment is a major-version suffix (`v` followed by a number of 2 or more) and another segment precedes it; then the alias SHALL be the preceding segment.

#### Scenario: Major-version module path
- **WHEN** a Go file has `import "github.com/go-chi/chi/v5"`
- **THEN** the recorded import alias is `chi`

#### Scenario: v1 path segment is a package name
- **WHEN** a Go file has `import "k8s.io/api/core/v1"`
- **THEN** the recorded import alias is `v1`

#### Scenario: Plain module path
- **WHEN** a Go file has `import "github.com/gin-gonic/gin"`
- **THEN** the recorded import alias is `gin`

#### Scenario: Router from a versioned module is detected
- **WHEN** a Go file imports `github.com/go-chi/chi/v5` and registers `router.Get("/items", h)` on `router := chi.NewMux()`
- **THEN** an endpoint with URI `/items` is extracted

### Requirement: Explicit import name wins
For an import with an explicit name other than `_` or `.`, the recorded alias SHALL be that name.

#### Scenario: Explicitly named versioned import
- **WHEN** a Go file has `import router "github.com/go-chi/chi/v5"`
- **THEN** the recorded import alias is `router`
