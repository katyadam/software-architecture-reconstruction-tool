# endpoint-route-prefixes Specification

## Purpose

Endpoints are reported with the full path a client must call, including every prefix added by router composition (sub-app mounts, router prefixes, router inclusion), in any language and framework that has a route-fact adapter, so REST calls can be matched to them without guessing.

## Requirements

### Requirement: Router composition prefixes are part of the endpoint URI
The system SHALL report an endpoint's URI as the concatenation, outermost first, of every prefix on the path from the top-level router to the router the endpoint is registered on, followed by the endpoint's own route.

#### Scenario: Sub-application mounted under a prefix
- **WHEN** a sub-application is attached to the top-level application under `/v3` and registers `GET /cases/{case_id}`
- **THEN** the endpoint URI is `/v3/cases/{case_id}`

#### Scenario: Router with its own prefix, included under another prefix
- **WHEN** a router declares its own prefix `/items`, registers `GET /{id}`, and is attached to the application under `/api`
- **THEN** the endpoint URI is `/api/items/{id}`

#### Scenario: Prefix given by a constant
- **WHEN** a router's prefix is the constant `BASE_PATH = "/api/v1/consignservice"` and the router registers `POST /consigns`
- **THEN** the endpoint URI is `/api/v1/consignservice/consigns`

### Requirement: Prefixes follow the router through function calls
The system SHALL resolve the router of an endpoint registered on a function parameter by following every call to that function and the argument passed at each call, across files of the same service.

#### Scenario: Router passed through two helper functions in different files
- **WHEN** the application module mounts `app_v1` at `/v1` and calls `add_routes_v1(app_v1)`, `add_routes_v1(app)` (another file) calls `add_routes_clinical(app)`, and `add_routes_clinical(app)` (a third file) registers `GET /cases/{case_id}` on `app`
- **THEN** the endpoint URI is `/v1/cases/{case_id}`

#### Scenario: Same helper name in two services
- **WHEN** two services each define and call their own `add_routes_v1`, mounted at different prefixes
- **THEN** each service's endpoints get only their own service's prefix

### Requirement: One endpoint per reachable prefix
The system SHALL report an endpoint once for each distinct full prefix through which it is reachable, and SHALL NOT drop any of them.

#### Scenario: Helper reused by two mounted sub-applications
- **WHEN** the same helper registering `GET /alive` is called with a sub-application mounted at `/v1` and with one mounted at `/v3`
- **THEN** two endpoints are reported, `/v1/alive` and `/v3/alive`

### Requirement: Unresolved prefixes keep today's URI
The system SHALL keep the endpoint URI unchanged when no route fact applies or a prefix value cannot be resolved to a string from any constant source, and SHALL terminate on recursive or cyclic call chains. When an endpoint is reachable through several prefix chains, only the chains containing an unresolvable prefix SHALL fall back to the declared URI; resolvable chains SHALL still produce prefixed endpoints.

#### Scenario: Prefix from an unknown expression
- **WHEN** a router's prefix is `settings.base`, which neither the project nor any external constant defines
- **THEN** the endpoint keeps its declared URI without a prefix

#### Scenario: One resolvable and one unresolvable chain
- **WHEN** a helper registering `GET /alive` is called with a sub-application mounted at `/v1` and with one mounted at an undefined `settings.base`
- **THEN** two endpoints are reported, `/v1/alive` and `/alive`

#### Scenario: Recursive helper
- **WHEN** a helper that registers routes calls itself with its own router parameter
- **THEN** extraction finishes and the endpoint gets the prefixes found on the non-recursive paths

### Requirement: Resolution is language and framework agnostic
The system SHALL resolve prefixes from a language-neutral description of router composition, so that supporting a new framework requires only describing its composition idioms, not changing the resolution. This change SHALL provide that description for Python FastAPI / Starlette (`mount`, `include_router`, `APIRouter(prefix=...)`).

#### Scenario: Same composition from neutral facts
- **WHEN** the resolver is given neutral facts "router `sub` attached to `app` under `/v3`" and an endpoint `GET /x` registered on `sub`, with no source language involved
- **THEN** the endpoint URI is `/v3/x`

#### Scenario: FastAPI idioms produce the facts
- **WHEN** a Python file contains `app.mount(path="/v3/fhir", app=app_v3)`, `app.include_router(r, prefix="/api")` and `r = APIRouter(prefix="/items")`
- **THEN** the facts are: `app_v3` attached to `app` under `/v3/fhir`, `r` attached to `app` under `/api`, `r` has own prefix `/items`

### Requirement: Languages without an adapter are unchanged
The system SHALL produce the same endpoints as before this change for languages and frameworks that report no route facts.

#### Scenario: train-ticket Java regression
- **WHEN** the CLI runs on train-ticket (Java) without `--llm` or `--typesafe`
- **THEN** the set of endpoint URIs equals the set produced before this change

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

### Requirement: Go changes keep corpus output stable
Adding Go route facts SHALL NOT change endpoint URIs, methods, counts or connections on corpora that use none of the Go composition idioms.

#### Scenario: Polyglot regression
- **WHEN** the CLI runs on train-ticket-polyglot without `--llm` or `--typesafe`
- **THEN** it reports the same 264 endpoints (service, method, URI) and 89 connections as before this change

### Requirement: Router composition can cross files through a module reference
The system SHALL resolve a router attached from another file when the attachment names the child by module, by searching from the attaching file's directory up through its ancestors and using the nearest match. A module with no match SHALL add no prefix. This resolution SHALL use only language-neutral facts.

#### Scenario: Neutral module attachment
- **WHEN** the resolver is given a neutral fact "router `routes` of module path `app/urls.py` attached to `root` under `api/`" from `svc/proj/urls.py`, and `svc/app/urls.py` registers `items/` on `routes`
- **THEN** the endpoint URI is `/api/items/`

#### Scenario: Nearest module wins
- **WHEN** `svc/proj/urls.py` attaches module path `app/urls.py`, and both `svc/app/urls.py` and `app/urls.py` exist
- **THEN** only routes in `svc/app/urls.py` get the prefix

#### Scenario: Same module path in another service
- **WHEN** `svc_a/proj/urls.py` attaches module path `accounts/urls.py`, and the only match is `svc_b/accounts/urls.py`
- **THEN** routes in `svc_b/accounts/urls.py` keep their URI

#### Scenario: Including file is not its own module
- **WHEN** `proj/api/urls.py` attaches module path `api/urls.py`, and `api/urls.py` also exists
- **THEN** routes in `api/urls.py` get the prefix

### Requirement: Django include produces prefixes
The system SHALL add the route of a Django `path(route, include("a.b"))` or `re_path(regex, include("a.b"))` entry as a prefix to every route in the `urlpatterns` of module `a.b`, whether that module is the file `a/b.py` or the package `a/b/__init__.py`. Nested includes SHALL join outermost first, and a module included more than once SHALL yield one endpoint per prefix. The `include()` entry itself SHALL still produce no endpoint.

#### Scenario: Include by module string
- **WHEN** `svc/proj/urls.py` declares `path("api/", include("app.urls"))` and `svc/app/urls.py` declares `path("items/", views.items)`
- **THEN** the endpoint URI is `/api/items/`
- **AND** no endpoint has the URI `api/`

#### Scenario: Module is a package
- **WHEN** `svc/proj/urls.py` declares `path("api/", include("app.urls"))` and `svc/app/urls/__init__.py` declares `path("items/", views.items)`
- **THEN** the endpoint URI is `/api/items/`

#### Scenario: Tuple form and namespace
- **WHEN** a file declares `path("a/", include(("app.urls", "app")))` or `path("a/", include("app.urls", namespace="x"))`
- **THEN** routes of `app.urls` get the prefix `a/`

#### Scenario: Nested includes
- **WHEN** `proj/urls.py` includes `app.urls` under `a/`, and `app/urls.py` includes `sub.urls` under `b/`, and `sub/urls.py` declares `path("x/", v)`
- **THEN** the endpoint URI is `/a/b/x/`

#### Scenario: Module included twice
- **WHEN** a file includes `app.urls` under `v1/` and under `v2/`, and `app/urls.py` declares `path("x/", v)`
- **THEN** the endpoint URIs are `/v1/x/` and `/v2/x/`

#### Scenario: Empty prefix
- **WHEN** a file declares `path("", include("app.urls"))` and `app/urls.py` declares `path("x/", v)`
- **THEN** the endpoint URI is `x/`

#### Scenario: Path converter in the prefix
- **WHEN** a file declares `path("<int:org>/", include("app.urls"))` and `app/urls.py` declares `path("x/", v)`
- **THEN** the endpoint URI is `/{org}/x/`

#### Scenario: Prefix from a constant in the including file
- **WHEN** `proj/urls.py` declares `API = "api/"` and `path(API, include("app.urls"))`, and `app/urls.py` declares `path("x/", v)`
- **THEN** the endpoint URI is `/api/x/`

#### Scenario: Plain regex prefix
- **WHEN** a file declares `re_path(r"^api/", include("app.urls"))` and `app/urls.py` declares `path("x/", v)`
- **THEN** the endpoint URI is `/api/x/`

#### Scenario: Regex prefix with metacharacters
- **WHEN** a file declares `re_path(r"^v(?P<n>\d+)/", include("app.urls"))` and `app/urls.py` declares `path("x/", v)`
- **THEN** the endpoint URI is `x/`

#### Scenario: View methods in an included module
- **WHEN** `app/urls.py` is included under `api/` and routes `path("x/", view)` to a view declared `@api_view(["GET", "POST"])`
- **THEN** both the GET and the POST endpoint have the URI `/api/x/`

#### Scenario: Module outside the project
- **WHEN** a file declares `path("health/", include("health_check.urls"))` and no such module exists in the project
- **THEN** no endpoint is added and other routes keep their URI

#### Scenario: Non-literal module argument
- **WHEN** a file declares `path("api/", include(router.urls))`
- **THEN** no endpoint is added and no prefix is applied

#### Scenario: Circular include
- **WHEN** `a/urls.py` includes `b.urls` and `b/urls.py` includes `a.urls`, and nothing else includes either
- **THEN** resolution terminates and routes in both files keep their URI

### Requirement: Java class-level prefixes come from the method's own type
The system SHALL take a Java endpoint's class-level prefix only from a `@RequestMapping` or `@Path` annotation on the nearest class or interface that encloses the method. It SHALL join that prefix to the method path with a single `/`. Other annotations on the type SHALL NOT affect the URI, and their order SHALL NOT matter.

#### Scenario: JAX-RS path with produces
- **WHEN** a class is annotated `@Path("/users") @Produces(MediaType.APPLICATION_JSON)` and has a method `@GET @Path("/{id}")`
- **THEN** the endpoint URI is `/users/{id}`

#### Scenario: Spring mapping followed by an annotation with a value
- **WHEN** a class is annotated `@RequestMapping("/api") @CrossOrigin("*")` and has a method `@GetMapping("/a")`
- **THEN** the endpoint URI is `/api/a`

#### Scenario: Spring mapping followed by a named-argument annotation
- **WHEN** a class is annotated `@RequestMapping("/api/v1") @Api(tags = "x")` and has a method `@GetMapping("/a")`
- **THEN** the endpoint URI is `/api/v1/a`

#### Scenario: Two classes in one file
- **WHEN** one file declares class `A` with `@RequestMapping("/one")` and method `@GetMapping("/a")`, and class `B` with `@RequestMapping("/two")` and method `@GetMapping("/b")`
- **THEN** the endpoint URIs are `/one/a` and `/two/b`

#### Scenario: Nested class
- **WHEN** a class with `@RequestMapping("/outer")` contains a nested class with `@RequestMapping("/inner")` and method `@GetMapping("/x")`
- **THEN** the endpoint URI is `/inner/x`

#### Scenario: Interface with a mapping
- **WHEN** an interface not annotated `@FeignClient` is annotated `@RequestMapping("/api")` and declares `@GetMapping("/x") String x();`
- **THEN** the endpoint URI is `/api/x`

#### Scenario: Slashes are normalized
- **WHEN** a class is annotated `@RequestMapping("/api/")` and has a method `@GetMapping("x")`
- **THEN** the endpoint URI is `/api/x`

#### Scenario: No class prefix
- **WHEN** a class has no `@RequestMapping` or `@Path` and has a method `@GetMapping("/a")`
- **THEN** the endpoint URI is `/a`

### Requirement: Java request mapping methods
The system SHALL take the HTTP method of a `@RequestMapping` method from its `method` attribute, reporting one endpoint per listed method. A `@RequestMapping` without `method` SHALL keep the method `GET`.

#### Scenario: Single method
- **WHEN** a method is annotated `@RequestMapping(value = "/a", method = RequestMethod.POST)`
- **THEN** the endpoint is `POST /a`

#### Scenario: Method array
- **WHEN** a method is annotated `@RequestMapping(value = "/a", method = {RequestMethod.GET, RequestMethod.PUT})`
- **THEN** the endpoints are `GET /a` and `PUT /a`

#### Scenario: No method attribute
- **WHEN** a method is annotated `@RequestMapping("/a")`
- **THEN** the endpoint is `GET /a`

### Requirement: Feign client interfaces are not endpoints
The system SHALL NOT report endpoints for methods of an interface annotated `@FeignClient`.

#### Scenario: Feign client
- **WHEN** an interface is annotated `@FeignClient("svc") @RequestMapping("/api")` and declares `@GetMapping("/x") String x();`
- **THEN** no endpoint is reported for `x`

### Requirement: Java path arrays give one endpoint per path
The system SHALL report one endpoint per element of a path array, at both class and method level. Both levels SHALL combine, so every class prefix is joined with every method path.

#### Scenario: Class prefix array
- **WHEN** a class is annotated `@RequestMapping({"/a", "/b"})` and has a method `@GetMapping("/x")`
- **THEN** the endpoint URIs are `/a/x` and `/b/x`

#### Scenario: Method path array
- **WHEN** a class has no prefix and a method is annotated `@GetMapping({"/x", "/y"})`
- **THEN** the endpoint URIs are `/x` and `/y`

#### Scenario: Both arrays
- **WHEN** a class is annotated `@RequestMapping({"/a", "/b"})` and a method `@GetMapping({"/x", "/y"})`
- **THEN** the endpoint URIs are `/a/x`, `/a/y`, `/b/x` and `/b/y`

### Requirement: Java class prefix constants resolve like other prefixes
The system SHALL resolve a Java class prefix written as a constant, bare or qualified by its class, to the constant's string value. A prefix that cannot be resolved SHALL be dropped, leaving the method path as the URI. A method-level path written as a constant SHALL keep its current text.

#### Scenario: Qualified constant
- **WHEN** a project declares `class Paths { static final String BASE = "/api"; }` and a controller is annotated `@RequestMapping(Paths.BASE)` with method `@GetMapping("/x")`
- **THEN** the endpoint URI is `/api/x`

#### Scenario: Same-class constant
- **WHEN** a controller declares `static final String BASE = "/api";`, is annotated `@RequestMapping(BASE)` and has method `@GetMapping("/x")`
- **THEN** the endpoint URI is `/api/x`

#### Scenario: Unresolved constant
- **WHEN** a controller is annotated `@RequestMapping(Config.base)`, which is not a resolvable string constant, and has method `@GetMapping("/x")`
- **THEN** the endpoint URI is `/x`

#### Scenario: Method-level constant unchanged
- **WHEN** a controller with no prefix has method `@GetMapping(Paths.ITEM)`
- **THEN** the endpoint URI is `Paths.ITEM`, as before this change

### Requirement: Endpoints without a router keep extractor-provided prefixes
The system SHALL keep the prefix chains an extractor records on an endpoint that has no router. Router-based resolution SHALL NOT clear them, including in projects that mix languages.

#### Scenario: Java and Python in one project
- **WHEN** a project contains a Python FastAPI service with route facts and a Java controller annotated `@RequestMapping("/api")` with method `@GetMapping("/x")`
- **THEN** the Java endpoint URI is `/api/x`

### Requirement: Java changes keep corpus output stable
The system SHALL produce the same endpoint count and the same service connections as before this change on train-ticket, train-ticket-polyglot and empaia. Endpoint URIs SHALL be equal except that a class prefix written without a leading `/` now yields a URI with one.

#### Scenario: train-ticket
- **WHEN** the CLI runs on train-ticket without `--llm` or `--typesafe`
- **THEN** the endpoint count and connections equal those before this change
- **AND** the only URI differences are `api/v1/...` becoming `/api/v1/...` for controllers whose `@RequestMapping` lacks a leading `/`

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

### Requirement: Prefix values resolve like REST call URLs
The system SHALL resolve each router prefix expression with the same constant sources and precedence used to resolve REST call URLs: constants scanned from the project, constants supplied externally at run time, class attribute defaults, and module-level bindings visible in the file where the prefix is written. A prefix that resolves for a REST call URL in the same file SHALL resolve to the same string for an endpoint.

#### Scenario: Prefix from an external constant
- **WHEN** a router's prefix is `settings.base`, the run is given external constant `settings.base = "/api"`, and the router registers `GET /items`
- **THEN** the endpoint URI is `/api/items`

#### Scenario: Prefix from a chained constant
- **WHEN** a file declares `ROOT = "/api"` and `BASE = ROOT`, and a router with prefix `BASE` registers `GET /items`
- **THEN** the endpoint URI is `/api/items`

#### Scenario: Prefix constant imported from another file
- **WHEN** `config.py` declares `PREFIX = "/v1"`, `app.py` imports `PREFIX` from `config` and mounts a sub-application under `PREFIX`, and the sub-application registers `GET /cases`
- **THEN** the endpoint URI is `/v1/cases`

#### Scenario: Same expression, same value
- **WHEN** a file uses constant `BASE` both as a router prefix and in `url = BASE + "/x"; requests.get(url)`
- **THEN** the endpoint prefix and the REST call URL use the same resolved value of `BASE`

### Requirement: File-local constants take precedence over project-wide constants
The system SHALL resolve a name in a file to that file's own module-level binding or a binding it imports before using a same-named constant from elsewhere in the project. This SHALL apply to router prefixes and REST call URLs alike. Same-named constants in different services SHALL NOT affect each other.

#### Scenario: Same constant name in two services
- **WHEN** service A's router file declares `BASE_PATH = "/api/v1/a"`, service B's declares `BASE_PATH = "/api/v1/b"`, and each registers `POST /items` on a router with prefix `BASE_PATH`
- **THEN** the endpoint URIs are `/api/v1/a/items` in A and `/api/v1/b/items` in B

#### Scenario: REST call URL uses its own file's constant
- **WHEN** two services each declare `BASE = ...` with different values and a file in the second service has `url = BASE + "/x"; requests.get(url)`
- **THEN** the REST call URL uses the second service's value

### Requirement: Ambiguous project constants do not resolve prefixes
The system SHALL NOT resolve a router prefix from a name that the file neither declares nor imports when that name has more than one distinct value across the project. The endpoint SHALL keep its declared URI, independent of file order.

#### Scenario: Unbound name declared twice elsewhere
- **WHEN** a router's prefix is `BASE`, its file does not declare or import `BASE`, and two other files declare `BASE` with different values
- **THEN** the endpoint keeps its declared URI in every file order

#### Scenario: Unbound name with one value
- **WHEN** a Go file in a package uses prefix `API_BASE` and another file of the package declares `const API_BASE = "/api"`
- **THEN** the endpoint URI starts with `/api`

### Requirement: Router fan-out is bounded
The system SHALL bound the prefix chains kept per endpoint to a fixed limit, so that memory stays bounded. Past the limit, the kept chains SHALL be chosen deterministically. This limit is the only exception to reporting one endpoint per reachable prefix. Time is not bounded by this limit.

#### Scenario: Wide router composition
- **WHEN** several levels of routers are each attached four times with distinct non-literal prefixes, exceeding the limit
- **THEN** the endpoint has at most the limit of chains, the same ones in every run

### Requirement: Endpoint output shape is unchanged
The system SHALL NOT add fields to the serialized endpoint output for prefix resolution. Intermediate prefix information SHALL stay internal to extraction.

#### Scenario: Serialized endpoint fields
- **WHEN** an endpoint with a resolved router prefix is written to extraction output
- **THEN** its serialized fields are the same set as an endpoint without a prefix
