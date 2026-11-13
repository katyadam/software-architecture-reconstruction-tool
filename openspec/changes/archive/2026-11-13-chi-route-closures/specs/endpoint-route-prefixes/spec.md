## ADDED Requirements

### Requirement: Go closure routers produce prefixes
The system SHALL extract a route registered on the parameter of a function literal passed to `.Route(prefix, closure)` or `.Group(closure)`, when the parameter's type comes from a known router package. The route SHALL be served under the outer router's prefixes, then every enclosing closure's prefix, outermost first. A `.Group` closure SHALL add no prefix. A parameter SHALL shadow a same-named outer router. A prefix that cannot be resolved SHALL leave the declared route unchanged.

#### Scenario: Closure shadows the outer router
- **WHEN** a Go function has `r := chi.NewRouter()` and `r.Route("/p", func(r chi.Router) { r.Get("/x", h) })`
- **THEN** the endpoint URI is `/p/x`

#### Scenario: Closure parameter with another name
- **WHEN** a Go function has `r := chi.NewRouter()` and `r.Route("/p", func(sub chi.Router) { sub.Get("/x", h) })`
- **THEN** the endpoint URI is `/p/x`

#### Scenario: Outer router is a parameter with a prefix
- **WHEN** `main` has `api := r.Group("/api")` and `routes(api)`, and `func routes(r chi.Router) { r.Route("/p", func(r chi.Router) { r.Get("/x", h) }) }`
- **THEN** the endpoint URI is `/api/p/x`

#### Scenario: Nested closures
- **WHEN** a Go function has `r.Route("/a", func(r chi.Router) { r.Route("/{id}", func(r chi.Router) { r.Get("/b", h) }) })`
- **THEN** the endpoint URI is `/a/{id}/b`

#### Scenario: Index route
- **WHEN** a Go function has `r.Route("/p", func(r chi.Router) { r.Get("/", h) })`
- **THEN** the endpoint URI is `/p`

#### Scenario: Outer closure router used from a nested closure
- **WHEN** a Go function has `r.Route("/a", func(a chi.Router) { a.Route("/b", func(b chi.Router) { a.Get("/x", h) }) })`
- **THEN** the endpoint URI is `/a/x`

#### Scenario: Chained closures stay separate
- **WHEN** a Go function has `r := chi.NewRouter()` and `r.Route("/a", func(r chi.Router) { r.Get("/x", h) }).Route("/b", func(r chi.Router) { r.Get("/y", h) })`
- **THEN** the endpoint URIs are `/a/x` and `/b/y`

#### Scenario: Group closure adds no prefix
- **WHEN** a Go function has `r.Route("/a", func(r chi.Router) { r.Group(func(r chi.Router) { r.Get("/x", h) }) })`
- **THEN** the endpoint URI is `/a/x`

#### Scenario: Prefix from a constant
- **WHEN** a Go file declares `const base = "/p"` and a function has `r.Route(base, func(r chi.Router) { r.Get("/x", h) })`
- **THEN** the endpoint URI is `/p/x`

#### Scenario: Unresolved prefix
- **WHEN** a Go function has `r.Route(cfg.Base, func(r chi.Router) { r.Get("/x", h) })`
- **THEN** the endpoint URI is `/x`

#### Scenario: Mount inside a closure
- **WHEN** a Go function has `admin := chi.NewRouter()`, `admin.Get("/stats", stats)` and `r.Route("/p", func(r chi.Router) { r.Mount("/admin", admin) })`
- **THEN** the endpoint URI is `/p/admin/stats`

#### Scenario: Closure typed with another package
- **WHEN** a Go function has `x.Route("/p", func(r Local) { r.Get("/x", h) })` and `Local` is a local type
- **THEN** no endpoint `GET /x` is reported

## MODIFIED Requirements

### Requirement: Go routes on a router parameter are extracted and prefixed
The system SHALL extract a route registered on a parameter of a named Go function or method when the parameter's type comes from a known router package (chi, Gin, Echo, Fiber, httprouter, gorilla mux, `net/http` ServeMux). The prefix SHALL come from every call to that function, following the argument passed, across files of the same service. A route registered on a parameter of a function literal (closure) SHALL NOT be extracted by this requirement, unless that closure is passed to `.Route` or `.Group`.

#### Scenario: Group passed to a helper in another file
- **WHEN** `main.go` has `r := gin.Default()`, `api := r.Group("/api")` and `registerRoutes(api)`, and `routes.go` in the same package has `func registerRoutes(g *gin.RouterGroup) { g.GET("/health", health) }`
- **THEN** the endpoint `GET /api/health` is reported

#### Scenario: chi router parameter with no caller
- **WHEN** a Go file has `func routes(r chi.Router) { r.Get("/status", status) }` and nothing calls `routes`
- **THEN** the endpoint `GET /status` is reported with router `r` and enclosing function `routes`

#### Scenario: ServeMux parameter on a method
- **WHEN** a Go file has `func (s *Server) register(mux *http.ServeMux) { mux.HandleFunc("GET /items", s.list) }`
- **THEN** the endpoint `GET /items` is reported with enclosing function `register`

#### Scenario: Parameter of an unrelated type
- **WHEN** a Go file has `func f(c *Cache) { c.Get("/key", v) }` and `Cache` is a local type
- **THEN** no endpoint is reported

#### Scenario: Closure parameter is not extracted
- **WHEN** a Go file has `func routes(r chi.Router) { s.Register(func(r chi.Router) { r.Get("/x", h) }) }`
- **THEN** no endpoint `GET /x` is reported
