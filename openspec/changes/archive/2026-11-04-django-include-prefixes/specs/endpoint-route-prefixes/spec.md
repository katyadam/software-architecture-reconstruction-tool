## ADDED Requirements

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
