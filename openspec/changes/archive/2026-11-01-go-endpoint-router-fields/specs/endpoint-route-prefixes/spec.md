## ADDED Requirements

### Requirement: Endpoints carry their registering router and function
The system SHALL record, for each endpoint extracted from a route-registration call, the router expression the route is registered on and the bare name of the function that contains the registration. The router SHALL be absent when the route is registered on an imported package rather than a router value. The function SHALL be absent when the registration is at module level. Recording these SHALL NOT change the endpoint's URI, method or handler.

#### Scenario: Go chi route inside a function
- **WHEN** a Go file has `func routes() http.Handler { r := chi.NewRouter(); r.Get("/status", h) }`
- **THEN** the endpoint `GET /status` has router `r` and enclosing function `routes`

#### Scenario: Go method receiver
- **WHEN** a Go file has `func (s *Server) register() { mux := http.NewServeMux(); mux.HandleFunc("GET /items", s.list) }`
- **THEN** the endpoint `GET /items` has router `mux` and enclosing function `register`

#### Scenario: Go gorilla chained registration
- **WHEN** a Go file has `func setup(r *mux.Router) { r.HandleFunc("/pay", Pay).Methods("POST") }`
- **THEN** the endpoint `POST /pay` has router `r` and enclosing function `setup`

#### Scenario: Go package-level registration
- **WHEN** a Go file has `func main() { http.HandleFunc("GET /health", h) }` with `net/http` imported as `http`
- **THEN** the endpoint has no router and enclosing function `main`

#### Scenario: Go registration inside a closure
- **WHEN** a Go file has `func routes() { r := chi.NewRouter(); register := func() { r.Get("/x", h) }; register() }`
- **THEN** the endpoint `GET /x` has router `r` and enclosing function `routes`

#### Scenario: Go URIs unchanged
- **WHEN** the CLI runs on train-ticket-polyglot without `--llm` or `--typesafe`
- **THEN** the set of endpoint URIs and the endpoint count (264) equal those before this change
