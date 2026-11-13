## ADDED Requirements

### Requirement: Go router composition produces prefixes
The system SHALL describe Go router composition to the agnostic resolver for these idioms: a router group created with a prefix (`x := r.Group("/p")`, Gin / Echo / Fiber), a gorilla subrouter under a path prefix (`x := r.PathPrefix("/p").Subrouter()`), and a sub-router mounted under a prefix (`r.Mount("/p", sub)`, chi / Fiber). A group prefix SHALL be applied exactly once, whether the group and its routes are in the same function or not. A prefix given by a constant declared in the same file SHALL be resolved to its value.

#### Scenario: Gin group in the same function
- **WHEN** a Go function has `router := gin.Default()`, `api := router.Group("/api")` and `api.GET("/customers/:id", getCustomer)`
- **THEN** the endpoint URI is `/api/customers/:id`

#### Scenario: Nested groups
- **WHEN** a Go function has `api := r.Group("/api")`, `v1 := api.Group("/v1")` and `v1.GET("/items", list)`, with `r := gin.New()`
- **THEN** the endpoint URI is `/api/v1/items`

#### Scenario: Gorilla subrouter
- **WHEN** a Go function has `r := mux.NewRouter()`, `s := r.PathPrefix("/orders").Subrouter()` and `s.HandleFunc("/{id}", get).Methods("GET")`
- **THEN** the endpoint URI is `/orders/{id}`

#### Scenario: chi mount of a named sub-router
- **WHEN** a Go function has `r := chi.NewRouter()`, `admin := chi.NewRouter()`, `admin.Get("/stats", stats)` and `r.Mount("/admin", admin)`
- **THEN** the endpoint URI is `/admin/stats`

#### Scenario: Group prefix from a constant
- **WHEN** a Go file declares `const apiBase = "/api"` and a function has `g := e.Group(apiBase)` and `g.POST("/carts", add)`, with `e := echo.New()`
- **THEN** the endpoint URI is `/api/carts`

#### Scenario: Mount of a call result is ignored
- **WHEN** a Go function has `r.Mount("/admin", adminRouter())`
- **THEN** no prefix is recorded for it and endpoints registered inside `adminRouter` keep their own route

### Requirement: Go routes on a router parameter are extracted and prefixed
The system SHALL extract a route registered on a parameter of a named Go function or method when the parameter's type comes from a known router package (chi, Gin, Echo, Fiber, httprouter, gorilla mux, `net/http` ServeMux). The prefix SHALL come from every call to that function, following the argument passed, across files of the same service. A route registered on a parameter of a function literal (closure) SHALL NOT be extracted by this requirement.

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
- **WHEN** a Go file has `func routes(r chi.Router) { r.Route("/p", func(r chi.Router) { r.Get("/x", h) }) }`
- **THEN** no endpoint `GET /x` is reported

### Requirement: Go changes keep corpus output stable
Adding Go route facts SHALL NOT change endpoint URIs, methods, counts or connections on corpora that use none of the Go composition idioms.

#### Scenario: Polyglot regression
- **WHEN** the CLI runs on train-ticket-polyglot without `--llm` or `--typesafe`
- **THEN** it reports the same 264 endpoints (service, method, URI) and 89 connections as before this change
