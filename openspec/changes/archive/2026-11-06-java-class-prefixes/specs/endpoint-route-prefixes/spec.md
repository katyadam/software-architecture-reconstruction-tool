## ADDED Requirements

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
