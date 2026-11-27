# Spec Delta

## Purpose

Defines how the Java extractor turns Spring MVC and JAX-RS annotations into endpoints with an HTTP method and URI.

## ADDED Requirements

### Requirement: Class prefix from route annotations only
The class-level URI prefix SHALL come only from a `@RequestMapping` or `@Path` annotation on the class or interface that directly declares the method. Other class annotations SHALL NOT affect the URI.

#### Scenario: JAX-RS class with @Produces
- **WHEN** a class has `@Path("/users") @Produces(MediaType.APPLICATION_JSON)` and a method `@GET @Path("/{id}")`
- **THEN** the endpoint URI is `/users/{id}`

#### Scenario: Spring class with @CrossOrigin
- **WHEN** a class has `@RequestMapping("/api") @CrossOrigin("*")` and a method `@GetMapping("/a")`
- **THEN** the endpoint URI is `/api/a`

#### Scenario: Spring class with @Api after @RequestMapping
- **WHEN** a class has `@RequestMapping("/api/v1") @Api(tags = "x")` and a method `@GetMapping("/a")`
- **THEN** the endpoint URI is `/api/v1/a`

#### Scenario: Two controllers in one file
- **WHEN** one file declares a class with `@RequestMapping("/one")` and another with `@RequestMapping("/two")`, each with `@GetMapping("/x")`
- **THEN** the endpoints are `/one/x` and `/two/x`

### Requirement: HTTP method from RequestMapping method argument
A `@RequestMapping` with a `method = RequestMethod.<VERB>` argument SHALL produce an endpoint with that verb, and its `value`/`path` argument SHALL be the URI.

#### Scenario: POST via RequestMapping
- **WHEN** a method has `@RequestMapping(value = "/a", method = RequestMethod.POST)`
- **THEN** the endpoint is `POST /a`

### Requirement: Feign clients are not endpoints
Methods declared in a type annotated `@FeignClient` SHALL NOT be reported as endpoints.

#### Scenario: Feign interface
- **WHEN** an interface has `@FeignClient(name = "x") @RequestMapping("/api")` and a method `@GetMapping("/x")`
- **THEN** no endpoint is reported

### Requirement: Multiple class prefixes
A class prefix given as an array SHALL produce one endpoint per prefix.

#### Scenario: Two prefixes
- **WHEN** a class has `@RequestMapping({"/a", "/b"})` and a method `@GetMapping("/x")`
- **THEN** the endpoints are `/a/x` and `/b/x`
