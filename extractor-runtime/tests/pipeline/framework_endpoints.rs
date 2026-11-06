use std::collections::HashMap;

use extractor_runtime::pipeline::{
    self,
    pass3::{pass_attr, pass_module},
};
use go_extractor::extraction::extract_syntactic as go_extract;
use java_extractor::extraction::extract_syntactic as java_extract;
use models::HttpMethod;
use python_extractor::extraction::parse::extract_syntactic as python_extract;

#[test]
fn django_urlpatterns_use_api_view_methods_from_view_functions() {
    let urls = r#"
from django.urls import path
from . import views

urlpatterns = [
    path("products/", views.get_products),
    path("products/create/", views.create_product),
]
"#;
    let views = r#"
from rest_framework.decorators import api_view

@api_view(["GET", "POST"])
def get_products(request):
    pass

@api_view(["POST"])
def create_product(request):
    pass
"#;

    let records = vec![
        python_extract(urls, "products/urls.py").expect("urls.py parses"),
        python_extract(views, "products/views.py").expect("views.py parses"),
    ];
    let project_ir = pipeline::build_project_ir(records);
    let endpoints = project_ir
        .files
        .iter()
        .flat_map(|file| file.endpoints.iter())
        .collect::<Vec<_>>();

    assert!(endpoints.iter().any(|endpoint| {
        endpoint.function_name == "get_products"
            && endpoint.uri == "products/"
            && endpoint.http_method == HttpMethod::GET
    }));
    assert!(endpoints.iter().any(|endpoint| {
        endpoint.function_name == "get_products"
            && endpoint.uri == "products/"
            && endpoint.http_method == HttpMethod::POST
    }));
    assert!(endpoints.iter().any(|endpoint| {
        endpoint.function_name == "create_product"
            && endpoint.uri == "products/create/"
            && endpoint.http_method == HttpMethod::POST
    }));
    assert!(
        endpoints
            .iter()
            .all(|endpoint| !(endpoint.uri.is_empty() && endpoint.router_variable.is_none()))
    );
}

#[test]
fn django_urlpatterns_handle_class_based_views_and_nested_includes() {
    let urls = r#"
from django.contrib import admin
from django.urls import include, path
from points.views import AwardView

urlpatterns = [
    path("admin/", admin.site.urls),
    path("health/", include("health_check.urls")),
    path("api/award/<uuid:customer_id>/", AwardView.as_view()),
]
"#;

    let record = python_extract(urls, "points/service/urls.py").expect("urls.py parses");
    let endpoint = record
        .endpoints
        .iter()
        .find(|endpoint| endpoint.function_name == "AwardView")
        .expect("class-based Django view should be extracted");

    assert_eq!(endpoint.http_method, HttpMethod::GET);
    assert_eq!(endpoint.uri, "api/award/{customer_id}/");
    assert_eq!(endpoint.router_variable.as_deref(), Some("urlpatterns"));
    assert!(
        record
            .endpoints
            .iter()
            .all(|endpoint| endpoint.uri != "health/")
    );
}

/// Passes 2 and 3, as both real callers run them.
fn evaluated(
    records: Vec<models::ir::syntax::FileRecord>,
    external_constants: &HashMap<String, String>,
) -> models::ir::evaluted::EvaluatedIR {
    let project_ir = pipeline::build_project_ir(records);
    let attrs = pass_attr::resolve_all(&project_ir, external_constants);
    let module_consts = pass_module::resolve_all(&project_ir, external_constants, &attrs);
    pipeline::evaluate(project_ir, external_constants, &attrs, &module_consts)
}

fn endpoint_uris(records: Vec<models::ir::syntax::FileRecord>) -> Vec<String> {
    uris_with(records, &HashMap::new())
}

fn uris_with(
    records: Vec<models::ir::syntax::FileRecord>,
    external_constants: &HashMap<String, String>,
) -> Vec<String> {
    let mut uris: Vec<String> = evaluated(records, external_constants)
        .endpoints
        .into_iter()
        .map(|e| e.uri)
        .collect();
    uris.sort();
    uris
}

fn py(code: &str, path: &str) -> models::ir::syntax::FileRecord {
    python_extract(code, path).expect("Python parses")
}

/// FastAPI router with `prefix` registering `POST /items`, preceded by `header`.
fn router_file(header: &str, prefix: &str) -> String {
    format!(
        r#"
from fastapi import APIRouter
{header}

router = APIRouter(prefix={prefix})

@router.post("/items")
async def create():
    return None
"#
    )
}

#[test]
fn fastapi_mount_prefix_follows_helper_calls() {
    let app = r#"
from fastapi import FastAPI
from .api.v1 import add_routes_v1

app = FastAPI()
app_v1 = FastAPI()
add_routes_v1(app_v1)
app.mount("/v1", app_v1)

@app.get("/alive")
async def alive():
    return {}
"#;
    let init = r#"
from .clinical import add_routes_clinical


def add_routes_v1(app):
    add_routes_clinical(app)
"#;
    let clinical = r#"
def add_routes_clinical(app):
    @app.get("/cases/{case_id}")
    async def _(case_id: str):
        return case_id
"#;
    let records = vec![
        python_extract(app, "svc/svc_pkg/app.py").expect("app.py parses"),
        python_extract(init, "svc/svc_pkg/api/v1/__init__.py").expect("__init__.py parses"),
        python_extract(clinical, "svc/svc_pkg/api/v1/clinical.py").expect("clinical.py parses"),
    ];
    assert_eq!(
        endpoint_uris(records),
        vec!["/alive", "/v1/cases/{case_id}"]
    );
}

#[test]
fn fastapi_router_own_prefix_from_module_constant() {
    let routes = r#"
from fastapi import APIRouter

BASE_PATH = "/api/v1/consignservice"


def create_router(service) -> APIRouter:
    router = APIRouter(prefix=BASE_PATH)

    @router.post("/consigns")
    async def insert():
        return None

    return router
"#;
    let records = vec![python_extract(routes, "svc/app/api/routes.py").expect("routes.py parses")];
    assert_eq!(
        endpoint_uris(records),
        vec!["/api/v1/consignservice/consigns"]
    );
}

#[test]
fn flask_blueprint_prefix_and_register_prefix() {
    let app = r#"
from flask import Flask, Blueprint

app = Flask(__name__)
bp = Blueprint("orders", __name__, url_prefix="/orders")

@bp.route("/<id>")
def get_order(id):
    return id

app.register_blueprint(bp, url_prefix="/api")
"#;
    let records = vec![python_extract(app, "svc/app.py").expect("app.py parses")];
    assert_eq!(endpoint_uris(records), vec!["/api/orders/{id}"]);
}

/// Pipeline endpoint URIs for Go `(path, code)` files.
fn go_endpoint_uris(files: &[(&str, &str)]) -> Vec<String> {
    endpoint_uris(
        files
            .iter()
            .map(|(path, code)| go_extract(code, path).expect("Go parses"))
            .collect(),
    )
}

#[test]
fn gin_group_prefix_applied_once() {
    let code = r#"
package main

func health() {}
func getCustomer() {}
func addItemToBasket() {}

func startRest() {
    router := gin.Default()
    routerGroup := router.Group("/api")
    routerGroup.GET("/customers/:id", getCustomer)
    routerGroup.POST("/customer-basket", addItemToBasket)
    routerGroup.GET("/health", health)
}
"#;
    assert_eq!(
        go_endpoint_uris(&[("svc/gin_group.go", code)]),
        vec!["/api/customer-basket", "/api/customers/:id", "/api/health"]
    );
}

#[test]
fn gin_nested_groups() {
    let code = r#"
package main

func startRest() {
    r := gin.New()
    api := r.Group("/api")
    v1 := api.Group("/v1")
    v1.GET("/items", list)
}
"#;
    assert_eq!(
        go_endpoint_uris(&[("svc/main.go", code)]),
        vec!["/api/v1/items"]
    );
}

#[test]
fn gorilla_subrouter_prefix() {
    let code = r#"
package main

import "github.com/gorilla/mux"

func setup() {
    r := mux.NewRouter()
    s := r.PathPrefix("/orders").Subrouter()
    s.HandleFunc("/{id}", get).Methods("GET")
}
"#;
    assert_eq!(
        go_endpoint_uris(&[("svc/main.go", code)]),
        vec!["/orders/{id}"]
    );
}

#[test]
fn chi_mount_named_subrouter() {
    let code = r#"
package main

func routes() {
    r := chi.NewRouter()
    admin := chi.NewRouter()
    admin.Get("/stats", stats)
    r.Mount("/admin", admin)
}
"#;
    assert_eq!(
        go_endpoint_uris(&[("svc/main.go", code)]),
        vec!["/admin/stats"]
    );
}

/// Endpoint URIs of a chi file; the import lets closure parameter types resolve.
fn chi_uris(decls: &str) -> Vec<String> {
    let code = format!("package main\n\nimport \"github.com/go-chi/chi/v5\"\n\n{decls}\n");
    go_endpoint_uris(&[("svc/main.go", code.as_str())])
}

#[test]
fn chi_closure_shadows_outer_router() {
    let uris = chi_uris(
        r#"func routes() {
    r := chi.NewRouter()
    r.Route("/p", func(r chi.Router) { r.Get("/x", h) })
}"#,
    );
    assert_eq!(uris, vec!["/p/x"]);
}

#[test]
fn chi_closure_parameter_with_another_name() {
    let uris = chi_uris(
        r#"func routes() {
    r := chi.NewRouter()
    r.Route("/p", func(sub chi.Router) { sub.Get("/x", h) })
}"#,
    );
    assert_eq!(uris, vec!["/p/x"]);
}

#[test]
fn chi_closure_outer_router_is_prefixed_parameter() {
    let uris = chi_uris(
        r#"func main() {
    r := chi.NewRouter()
    api := r.Group("/api")
    routes(api)
}

func routes(r chi.Router) {
    r.Route("/p", func(r chi.Router) { r.Get("/x", h) })
}"#,
    );
    assert_eq!(uris, vec!["/api/p/x"]);
}

#[test]
fn chi_nested_closures() {
    let uris = chi_uris(
        r#"func routes() {
    r := chi.NewRouter()
    r.Route("/a", func(r chi.Router) {
        r.Route("/{id}", func(r chi.Router) { r.Get("/b", h) })
    })
}"#,
    );
    assert_eq!(uris, vec!["/a/{id}/b"]);
}

#[test]
fn chi_closure_index_route() {
    let uris = chi_uris(
        r#"func routes() {
    r := chi.NewRouter()
    r.Route("/p", func(r chi.Router) { r.Get("/", h) })
}"#,
    );
    assert_eq!(uris, vec!["/p"]);
}

#[test]
fn chi_outer_closure_router_used_from_nested_closure() {
    let uris = chi_uris(
        r#"func routes() {
    r := chi.NewRouter()
    r.Route("/a", func(a chi.Router) {
        a.Route("/b", func(b chi.Router) { a.Get("/x", h) })
    })
}"#,
    );
    assert_eq!(uris, vec!["/a/x"]);
}

#[test]
fn chi_group_closure_adds_no_prefix() {
    let uris = chi_uris(
        r#"func routes() {
    r := chi.NewRouter()
    r.Route("/a", func(r chi.Router) {
        r.Group(func(r chi.Router) { r.Get("/x", h) })
    })
}"#,
    );
    assert_eq!(uris, vec!["/a/x"]);
}

#[test]
fn chi_group_closure_parameter_with_another_name() {
    let uris = chi_uris(
        r#"func routes() {
    r := chi.NewRouter()
    r.Route("/a", func(r chi.Router) {
        r.Group(func(sub chi.Router) { sub.Get("/x", h) })
    })
}"#,
    );
    assert_eq!(uris, vec!["/a/x"]);
}

#[test]
fn chi_closure_prefix_from_const() {
    let uris = chi_uris(
        r#"const base = "/p"

func routes() {
    r := chi.NewRouter()
    r.Route(base, func(r chi.Router) { r.Get("/x", h) })
}"#,
    );
    assert_eq!(uris, vec!["/p/x"]);
}

#[test]
fn chi_closure_unresolved_prefix() {
    let uris = chi_uris(
        r#"func routes() {
    r := chi.NewRouter()
    r.Route(cfg.Base, func(r chi.Router) { r.Get("/x", h) })
}"#,
    );
    assert_eq!(uris, vec!["/x"]);
}

#[test]
fn chi_closure_unresolved_identifier_prefix() {
    let uris = chi_uris(
        r#"func routes(base string) {
    r := chi.NewRouter()
    r.Route(base, func(r chi.Router) { r.Get("/x", h) })
}"#,
    );
    assert_eq!(uris, vec!["/x"]);
}

#[test]
fn chi_mount_inside_closure() {
    let uris = chi_uris(
        r#"func routes() {
    r := chi.NewRouter()
    admin := chi.NewRouter()
    admin.Get("/stats", stats)
    r.Route("/p", func(r chi.Router) { r.Mount("/admin", admin) })
}"#,
    );
    assert_eq!(uris, vec!["/p/admin/stats"]);
}

#[test]
fn closure_typed_with_another_package_not_extracted() {
    let uris = chi_uris(
        r#"func routes(x *Thing) {
    x.Route("/p", func(r Local) { r.Get("/x", h) })
}"#,
    );
    assert_eq!(uris, Vec::<String>::new());
}

#[test]
fn fiber_route_closure() {
    let code = r#"
package main

import "github.com/gofiber/fiber/v2"

func routes() {
    app := fiber.New()
    app.Route("/p", func(r fiber.Router) { r.Get("/x", h) })
}
"#;
    assert_eq!(go_endpoint_uris(&[("svc/main.go", code)]), vec!["/p/x"]);
}

#[test]
fn chi_closure_in_method_with_router_parameter() {
    let uris = chi_uris(
        r#"func (s *S) routes(r chi.Router) {
    r.Route("/p", func(r chi.Router) { r.Get("/x", s.h) })
}"#,
    );
    assert_eq!(uris, vec!["/p/x"]);
}

#[test]
fn chi_closure_in_method_called_with_prefixed_router() {
    let uris = chi_uris(
        r#"func main() {
    r := chi.NewRouter()
    api := r.Group("/api")
    s := &S{}
    s.routes(api)
}

func (s *S) routes(r chi.Router) {
    r.Route("/p", func(r chi.Router) { r.Get("/x", s.h) })
}"#,
    );
    assert_eq!(uris, vec!["/api/p/x"]);
}

#[test]
fn chi_sibling_closures_and_outer_route() {
    let mut uris = chi_uris(
        r#"func routes() {
    r := chi.NewRouter()
    r.Route("/a", func(r chi.Router) { r.Get("/x", h) })
    r.Route("/b", func(r chi.Router) { r.Get("/y", h) })
    r.Get("/z", h)
}"#,
    );
    uris.sort();
    assert_eq!(uris, vec!["/a/x", "/b/y", "/z"]);
}

#[test]
fn chi_method_call_in_closure() {
    let uris = chi_uris(
        r#"func routes() {
    r := chi.NewRouter()
    r.Route("/p", func(r chi.Router) { r.Method("GET", "/x", h) })
}"#,
    );
    assert_eq!(uris, vec!["/p/x"]);
}

#[test]
fn echo_group_prefix_from_const() {
    let code = r#"
package main

const apiBase = "/api"

func startRest() {
    e := echo.New()
    g := e.Group(apiBase)
    g.POST("/carts", add)
}
"#;
    assert_eq!(
        go_endpoint_uris(&[("svc/main.go", code)]),
        vec!["/api/carts"]
    );
}

#[test]
fn chi_mount_call_result_ignored() {
    let code = r#"
package main

func adminRouter() chi.Router {
    r := chi.NewRouter()
    r.Get("/stats", stats)
    return r
}

func main() {
    r := chi.NewRouter()
    r.Mount("/admin", adminRouter())
}
"#;
    assert_eq!(go_endpoint_uris(&[("svc/main.go", code)]), vec!["/stats"]);
}

#[test]
fn gin_group_passed_to_helper_in_other_file() {
    let main = r#"
package main

func main() {
    r := gin.Default()
    api := r.Group("/api")
    registerRoutes(api)
}
"#;
    let routes = r#"
package main

import "github.com/gin-gonic/gin"

func registerRoutes(g *gin.RouterGroup) { g.GET("/health", health) }
"#;
    assert_eq!(
        go_endpoint_uris(&[("svc/main.go", main), ("svc/routes.go", routes)]),
        vec!["/api/health"]
    );
}

#[test]
fn gin_package_level_group_var() {
    let code = r#"
package main

var api *gin.RouterGroup

func init() {
    r := gin.Default()
    api = r.Group("/api")
}

func reg() { api.GET("/x", h) }
"#;
    assert_eq!(go_endpoint_uris(&[("svc/main.go", code)]), vec!["/api/x"]);
}

#[test]
fn prefix_from_external_constant() {
    let external = HashMap::from([("settings.base".to_string(), "/api".to_string())]);
    let code = router_file("", "settings.base");
    assert_eq!(
        uris_with(vec![py(&code, "svc/app.py")], &external),
        vec!["/api/items"]
    );
}

#[test]
fn prefix_from_chained_constant() {
    let code = router_file("ROOT = \"/api\"\nBASE = ROOT", "BASE");
    assert_eq!(
        endpoint_uris(vec![py(&code, "svc/app.py")]),
        vec!["/api/items"]
    );
}

#[test]
fn prefix_constant_imported_from_other_file() {
    let config = "PREFIX = \"/v1\"\n";
    let app = r#"
from fastapi import FastAPI
from config import PREFIX

app = FastAPI()
sub = FastAPI()

@sub.get("/cases")
async def cases():
    return []

app.mount(PREFIX, sub)
"#;
    assert_eq!(
        endpoint_uris(vec![py(config, "config.py"), py(app, "app.py")]),
        vec!["/v1/cases"]
    );
}

#[test]
fn prefix_and_rest_call_url_share_constant_value() {
    let code = r#"
import requests
from fastapi import APIRouter

BASE = "/shared"
router = APIRouter(prefix=BASE)

@router.get("/items")
async def items():
    return None

def call():
    url = BASE + "/x"
    requests.get(url)
"#;
    let ir = evaluated(vec![py(code, "svc/app.py")], &HashMap::new());
    assert_eq!(ir.endpoints[0].uri, "/shared/items");
    assert_eq!(ir.restcalls[0].target_uri, "/shared/x");
}

#[test]
fn same_constant_name_in_two_services_prefixes() {
    // Both orders: `collect_constants` is first-wins.
    let a = router_file("BASE_PATH = \"/api/v1/a\"", "BASE_PATH");
    let b = router_file("BASE_PATH = \"/api/v1/b\"", "BASE_PATH");
    for (first, second) in [
        (py(&a, "a/routes.py"), py(&b, "b/routes.py")),
        (py(&b, "b/routes.py"), py(&a, "a/routes.py")),
    ] {
        assert_eq!(
            endpoint_uris(vec![first, second]),
            vec!["/api/v1/a/items", "/api/v1/b/items"]
        );
    }
}

#[test]
fn same_constant_name_in_two_services_rest_calls() {
    let client = |base: &str| {
        format!(
            "import requests\n\nBASE = \"{base}\"\n\ndef call():\n    url = BASE + \"/x\"\n    requests.get(url)\n"
        )
    };
    let (a, b) = (client("http://a"), client("http://b"));
    for (first, second) in [
        (py(&a, "a/client.py"), py(&b, "b/client.py")),
        (py(&b, "b/client.py"), py(&a, "a/client.py")),
    ] {
        let ir = evaluated(vec![first, second], &HashMap::new());
        let of = |path: &str| {
            ir.restcalls
                .iter()
                .find(|r| r.file_path == path)
                .map(|r| r.target_uri.clone())
        };
        assert_eq!(of("a/client.py").as_deref(), Some("http://a/x"));
        assert_eq!(of("b/client.py").as_deref(), Some("http://b/x"));
    }
}

/// Target of the single REST call in `code`, through pass 3.
fn rest_target(code: &str) -> String {
    let ir = evaluated(vec![py(code, "svc/client.py")], &HashMap::new());
    assert_eq!(ir.restcalls.len(), 1, "{code}");
    ir.restcalls[0].target_uri.clone()
}

/// `requests.get(<target>)` in a function, under `header`.
fn inline_client(header: &str, target: &str) -> String {
    format!("import requests\n\n{header}\n\ndef call(item):\n    requests.get({target})\n")
}

#[test]
fn inline_concat_constant_plus_literal() {
    let code = inline_client("BASE = \"http://svc:8000\"", "BASE + \"/x\"");
    assert_eq!(rest_target(&code), "http://svc:8000/x");
}

#[test]
fn inline_concat_single_quoted_literal() {
    let code = inline_client("BASE = \"http://svc:8000\"", "BASE + '/x'");
    assert_eq!(rest_target(&code), "http://svc:8000/x");
}

#[test]
fn inline_concat_literal_between_names() {
    let code = inline_client(
        "BASE = \"http://svc:8000\"\nTAIL = \"y\"",
        "BASE + \"/x/\" + TAIL",
    );
    assert_eq!(rest_target(&code), "http://svc:8000/x/y");
}

#[test]
fn inline_concat_fstring_operand() {
    let code = inline_client("BASE = \"http://svc:8000\"", "BASE + f\"/x/{item}\"");
    assert_eq!(rest_target(&code), "http://svc:8000/x/{item}");
}

#[test]
fn inline_concat_fstring_prefix_variants() {
    for operand in ["rf\"/x/{item}\"", "F\"/x/{item}\"", "f'''/x/{item}'''"] {
        let code = inline_client("BASE = \"http://svc:8000\"", &format!("BASE + {operand}"));
        assert_eq!(rest_target(&code), "http://svc:8000/x/{item}", "{operand}");
    }
}

#[test]
fn inline_concat_unresolved_operand_stays_as_text() {
    let inline = "import requests\n\nclass Client:\n    def call(self):\n        requests.get(self.base + \"/x\")\n";
    assert_eq!(rest_target(inline), "{self.base}/x");
}

#[test]
fn inline_concat_constant_holding_fstring() {
    let code = inline_client(
        "HOST = \"svc\"\nBASE = f\"http://{HOST}:8000\"",
        "BASE + \"/x\"",
    );
    assert_eq!(rest_target(&code), "http://svc:8000/x");
}

/// Value starts with `f`, but is no f-string.
#[test]
fn constant_starting_with_f_keeps_its_value() {
    let header = "HOST = \"frontend:8000\"";
    let inline = inline_client(header, "HOST + \"/x\"");
    let variable = format!(
        "import requests\n\n{header}\n\ndef call():\n    url = HOST + \"/x\"\n    requests.get(url)\n"
    );
    let fstring = inline_client(header, "f\"{HOST}/x\"");
    // No `/` in the literal: identification drops it before pass 3.
    let bare_fstring = inline_client(header, "f\"{HOST}\"");
    assert_eq!(rest_target(&inline), "frontend:8000/x");
    assert_eq!(rest_target(&variable), "frontend:8000/x");
    assert_eq!(rest_target(&fstring), "frontend:8000/x");
    let ir = evaluated(vec![py(&bare_fstring, "svc/client.py")], &HashMap::new());
    assert!(ir.restcalls.is_empty(), "{:?}", ir.restcalls);
}

/// Leading hole: the matcher gives a suffix match, a literal first segment gets none.
#[test]
fn inline_concat_unresolved_local_becomes_hole() {
    let code = "import requests\n\ndef call(svc_url, name):\n    requests.get(svc_url + \"/a/b/\" + name)\n";
    assert_eq!(rest_target(code), "{svc_url}/a/b/{name}");
}

#[test]
fn inline_concat_other_operands_keep_todays_target() {
    let lookup = inline_client("", "lookup(item) + \"/y\"");
    assert_eq!(rest_target(&lookup), "/y");
    let formatted = inline_client(INLINE_HEADER, "BASE + \"/a/%s\" % item");
    assert_eq!(rest_target(&formatted), "\"/a/%s\" % item");
}

#[test]
fn inline_concat_same_constant_name_in_two_services() {
    let a = inline_client("BASE = \"http://a\"", "BASE + \"/x\"");
    let b = inline_client("BASE = \"http://b\"", "BASE + \"/x\"");
    for (first, second) in [
        (py(&a, "a/client.py"), py(&b, "b/client.py")),
        (py(&b, "b/client.py"), py(&a, "a/client.py")),
    ] {
        let ir = evaluated(vec![first, second], &HashMap::new());
        let of = |path: &str| {
            ir.restcalls
                .iter()
                .find(|r| r.file_path == path)
                .map(|r| r.target_uri.clone())
        };
        assert_eq!(of("a/client.py").as_deref(), Some("http://a/x"));
        assert_eq!(of("b/client.py").as_deref(), Some("http://b/x"));
    }
}

const INLINE_HEADER: &str = "BASE = \"http://svc:8000\"\nTAIL = \"y\"";

#[test]
fn inline_concat_equals_variable_form() {
    for target in [
        "BASE + \"/x\"",
        "BASE + '/x'",
        "BASE + \"/x/\" + TAIL",
        "BASE + f\"/x/{item}\"",
    ] {
        let variable = format!(
            "import requests\n\n{INLINE_HEADER}\n\ndef call(item):\n    url = {target}\n    requests.get(url)\n"
        );
        assert_eq!(
            rest_target(&inline_client(INLINE_HEADER, target)),
            rest_target(&variable),
            "{target}"
        );
    }
}

#[test]
fn inline_concat_multiline_call() {
    for target in [
        "\n        BASE + \"/x\"\n    ",
        "\n        BASE\n        + \"/x\"\n    ",
        "\n        BASE + \"/x\",  # it's a comment\n        timeout=5,\n    ",
    ] {
        let code = inline_client(INLINE_HEADER, target);
        assert_eq!(rest_target(&code), "http://svc:8000/x", "{target}");
    }
}

#[test]
fn inline_concat_with_keyword_arguments() {
    for target in [
        "BASE + \"/x\", timeout=5",
        "BASE + \"/x\", headers={\"a\": \"b+c\"}, timeout=5",
        "url=BASE + \"/x\", timeout=5",
    ] {
        let code = inline_client(INLINE_HEADER, target);
        assert_eq!(rest_target(&code), "http://svc:8000/x", "{target}");
    }
}

#[test]
fn inline_concat_non_ascii() {
    for (target, expected) in [
        ("BASE + \"/ñandú/é\"", "http://svc:8000/ñandú/é"),
        ("BASE + '/日本語/' + TAIL", "http://svc:8000/日本語/y"),
        ("BASE + f\"/ñ/{item}\"", "http://svc:8000/ñ/{item}"),
        ("é + \"/ñ\"", "{é}/ñ"),
    ] {
        let code = inline_client(INLINE_HEADER, target);
        assert_eq!(rest_target(&code), expected, "{target}");
    }
}

#[test]
fn inline_concat_literal_with_quote_or_bracket() {
    for (target, expected) in [
        ("BASE + \"/it's\"", "http://svc:8000/it's"),
        ("BASE + '/say\"hi\"'", "http://svc:8000/say\"hi\""),
        ("BASE + \"/x(\" + TAIL", "http://svc:8000/x(y"),
        ("BASE + \"/x)\" + TAIL", "http://svc:8000/x)y"),
    ] {
        let code = inline_client(INLINE_HEADER, target);
        assert_eq!(rest_target(&code), expected, "{target}");
    }
}

/// Tree-sitter recovery decides the target, so only survival is asserted.
#[test]
fn inline_concat_unterminated_quote_does_not_panic() {
    for target in [
        "BASE + \"/x",
        "BASE + '/é",
        "é + \"\"\"/x",
        "BASE + f\"/{item",
    ] {
        let code = format!("import requests\n\ndef call(item):\n    requests.get({target})\n");
        evaluated(vec![py(&code, "svc/client.py")], &HashMap::new());
    }
}

#[test]
fn inline_concat_dictionary_lookup_not_reported_after_pass3() {
    let code = "PREFIX = \"p\"\n\ndef call(settings):\n    settings.get(PREFIX + \"_key\")\n";
    let ir = evaluated(vec![py(code, "svc/client.py")], &HashMap::new());
    assert!(ir.restcalls.is_empty(), "{:?}", ir.restcalls);
}

#[test]
fn prefix_and_inline_rest_call_share_constant_value() {
    let code = r#"
import requests
from fastapi import APIRouter

BASE = "/shared"
router = APIRouter(prefix=BASE)

@router.get("/items")
async def items():
    return None

def call():
    requests.get(BASE + "/x")
"#;
    let ir = evaluated(vec![py(code, "svc/app.py")], &HashMap::new());
    assert_eq!(ir.endpoints[0].uri, "/shared/items");
    assert_eq!(ir.restcalls[0].target_uri, "/shared/x");
}

#[test]
fn imported_constant_beats_same_name_in_other_service() {
    let other = router_file("BASE_PATH = \"/other\"", "BASE_PATH");
    let config = "BASE_PATH = \"/mine\"\n";
    let routes = router_file("from svc.config import BASE_PATH", "BASE_PATH");
    assert_eq!(
        endpoint_uris(vec![
            py(&other, "other/routes.py"),
            py(config, "svc/config.py"),
            py(&routes, "svc/routes.py"),
        ]),
        vec!["/mine/items", "/other/items"]
    );
}

/// `ROOT` must come from this service's config, also inside `BASE = ROOT`.
#[test]
fn chained_through_import_ignores_same_name_in_other_service() {
    let other = "ROOT = \"/other\"\n";
    let config = "ROOT = \"/mine\"\n";
    let routes = router_file("from svc.config import ROOT\nBASE = ROOT", "BASE");
    assert_eq!(
        endpoint_uris(vec![
            py(other, "other/config.py"),
            py(config, "svc/config.py"),
            py(&routes, "svc/routes.py"),
        ]),
        vec!["/mine/items"]
    );
}

#[test]
fn one_resolvable_one_unresolvable_chain() {
    let app = r#"
from fastapi import FastAPI
from routes import add_routes

app = FastAPI()
app_v1 = FastAPI()
app_x = FastAPI()
add_routes(app_v1)
add_routes(app_x)
app.mount("/v1", app_v1)
app.mount(settings.base, app_x)
"#;
    let routes = r#"
def add_routes(app):
    @app.get("/alive")
    async def alive():
        return {}
"#;
    assert_eq!(
        endpoint_uris(vec![py(app, "app.py"), py(routes, "routes.py")]),
        vec!["/alive", "/v1/alive"]
    );
}

#[test]
fn unknown_prefix_keeps_uri() {
    let code = router_file("", "settings.base");
    assert_eq!(endpoint_uris(vec![py(&code, "svc/app.py")]), vec!["/items"]);
}

/// Project constants keep unevaluable right-hand sides as raw text.
#[test]
fn constant_without_string_value_keeps_uri() {
    for header in [
        "BASE = settings.base",
        "BASE = os.getenv(\"P\", \"/p\")",
        "BASE = OTHER",
        "BASE = 8080",
        "BASE = f\"/x/{OTHER}\"",
    ] {
        let code = router_file(header, "BASE");
        assert_eq!(
            endpoint_uris(vec![py(&code, "svc/app.py")]),
            vec!["/items"],
            "{header}"
        );
    }
}

#[test]
fn recursive_helper_keeps_non_recursive_prefix() {
    let app = r#"
from fastapi import FastAPI
from helper import h

app = FastAPI()
sub = FastAPI()
h(sub)
app.mount("/v1", sub)
"#;
    let helper = r#"
def h(app):
    h(app)

    @app.get("/x")
    async def x():
        return {}
"#;
    assert_eq!(
        endpoint_uris(vec![py(app, "app.py"), py(helper, "helper.py")]),
        vec!["/v1/x"]
    );
}

#[test]
fn serialized_endpoint_fields_unchanged_by_prefix() {
    let routed = router_file("", "\"/api\"");
    let plain = router_file("", "\"\"");
    let keys = |code: &str| {
        let ir = evaluated(vec![py(code, "svc/app.py")], &HashMap::new());
        let json = serde_json::to_value(&ir.endpoints[0]).expect("endpoint serializes");
        let mut keys: Vec<String> = json
            .as_object()
            .expect("endpoint is an object")
            .keys()
            .cloned()
            .collect();
        keys.sort();
        (ir.endpoints[0].uri.clone(), keys)
    };
    let (routed_uri, routed_keys) = keys(&routed);
    let (plain_uri, plain_keys) = keys(&plain);
    assert_eq!(routed_uri, "/api/items");
    assert_eq!(plain_uri, "/items");
    assert_eq!(routed_keys, plain_keys);
    assert!(!routed_keys.iter().any(|k| k == "route_prefixes"));
}

/// Own unresolvable binding must not fall through to another service's value.
#[test]
fn own_unresolvable_binding_ignores_other_service() {
    let a = router_file("BASE = \"/a\"", "BASE");
    for header in [
        "BASE = settings.base",
        "BASE = os.getenv(\"P\")",
        "BASE = unknown_fn()",
        "BASE = OTHER",
    ] {
        let b = router_file(header, "BASE");
        for (first, second) in [
            (py(&a, "a/routes.py"), py(&b, "b/routes.py")),
            (py(&b, "b/routes.py"), py(&a, "a/routes.py")),
        ] {
            assert_eq!(
                endpoint_uris(vec![first, second]),
                vec!["/a/items", "/items"],
                "{header}"
            );
        }
    }
}

#[test]
fn same_constant_name_in_two_go_services_rest_calls() {
    let client = |base: &str| {
        format!(
            "package main\n\nimport \"net/http\"\n\nconst BASE = \"{base}\"\n\nfunc call() {{\n    url := BASE + \"/x\"\n    http.Get(url)\n}}\n"
        )
    };
    let records = vec![
        go_extract(&client("http://a"), "a/c.go").expect("Go parses"),
        go_extract(&client("http://b"), "b/c.go").expect("Go parses"),
    ];
    let ir = evaluated(records, &HashMap::new());
    let mut urls: Vec<(&str, &str)> = ir
        .restcalls
        .iter()
        .map(|r| (r.file_path.as_str(), r.target_uri.as_str()))
        .collect();
    urls.sort();
    assert_eq!(
        urls,
        vec![("a/c.go", "http://a/x"), ("b/c.go", "http://b/x")]
    );
}

#[test]
fn fastapi_router_own_prefix_included_under_other_prefix() {
    let code = r#"
from fastapi import FastAPI, APIRouter

app = FastAPI()
r = APIRouter(prefix="/items")

@r.get("/{id}")
async def get(id: str):
    return id

app.include_router(r, prefix="/api")
"#;
    assert_eq!(
        endpoint_uris(vec![py(code, "svc/app.py")]),
        vec!["/api/items/{id}"]
    );
}

#[test]
fn fastapi_mount_keyword_arguments() {
    let code = r#"
from fastapi import FastAPI

app = FastAPI()
app_v3 = FastAPI()

@app_v3.get("/cases/{case_id}")
async def case(case_id: str):
    return case_id

app.mount(path="/v3/fhir", app=app_v3)
"#;
    assert_eq!(
        endpoint_uris(vec![py(code, "svc/app.py")]),
        vec!["/v3/fhir/cases/{case_id}"]
    );
}

#[test]
fn fastapi_helper_reused_by_two_mounts() {
    let app = r#"
from fastapi import FastAPI
from routes import add_routes

app = FastAPI()
app_v1 = FastAPI()
app_v3 = FastAPI()
add_routes(app_v1)
add_routes(app_v3)
app.mount("/v1", app_v1)
app.mount("/v3", app_v3)
"#;
    let routes = r#"
def add_routes(app):
    @app.get("/alive")
    async def alive():
        return {}
"#;
    assert_eq!(
        endpoint_uris(vec![py(app, "app.py"), py(routes, "routes.py")]),
        vec!["/v1/alive", "/v3/alive"]
    );
}

#[test]
fn fastapi_same_helper_name_in_two_services() {
    let app = |prefix: &str, pkg: &str| {
        format!(
            "from fastapi import FastAPI\nfrom {pkg}.routes import add_routes_v1\n\napp = FastAPI()\nsub = FastAPI()\nadd_routes_v1(sub)\napp.mount(\"{prefix}\", sub)\n"
        )
    };
    let routes = |route: &str| {
        format!(
            "def add_routes_v1(app):\n    @app.get(\"{route}\")\n    async def h():\n        return {{}}\n"
        )
    };
    assert_eq!(
        endpoint_uris(vec![
            py(&app("/a", "a"), "a/app.py"),
            py(&routes("/x"), "a/routes.py"),
            py(&app("/b", "b"), "b/app.py"),
            py(&routes("/y"), "b/routes.py"),
        ]),
        vec!["/a/x", "/b/y"]
    );
}

#[test]
fn go_group_backtick_literal_prefix() {
    let code = "package main\n\nfunc start() {\n    r := gin.New()\n    g := r.Group(`/api`)\n    g.GET(\"/items\", list)\n}\n";
    assert_eq!(
        go_endpoint_uris(&[("svc/main.go", code)]),
        vec!["/api/items"]
    );
}

#[test]
fn go_group_unresolved_identifier_keeps_uri() {
    let code = r#"
package main

func start() {
    e := echo.New()
    g := e.Group(apiBase)
    g.POST("/carts", add)
    v1 := g.Group("/v1")
    v1.GET("/items", list)
}
"#;
    assert_eq!(
        go_endpoint_uris(&[("svc/main.go", code)]),
        vec!["/carts", "/items"]
    );
}

#[test]
fn go_same_const_name_in_two_services_either_order() {
    let service = |base: &str| {
        format!(
            "package main\n\nconst API_BASE = \"{base}\"\n\nfunc start() {{\n    e := echo.New()\n    g := e.Group(API_BASE)\n    g.POST(\"/carts\", add)\n}}\n"
        )
    };
    let (a, b) = (service("/a"), service("/b"));
    for files in [
        [("a/main.go", a.as_str()), ("b/main.go", b.as_str())],
        [("b/main.go", b.as_str()), ("a/main.go", a.as_str())],
    ] {
        assert_eq!(go_endpoint_uris(&files), vec!["/a/carts", "/b/carts"]);
    }
}

#[test]
fn gin_same_helper_name_in_two_services() {
    let main = |prefix: &str| {
        format!(
            "package main\n\nfunc main() {{\n    r := gin.Default()\n    api := r.Group(\"{prefix}\")\n    registerRoutes(api)\n}}\n"
        )
    };
    let routes = "package main\n\nimport \"github.com/gin-gonic/gin\"\n\nfunc registerRoutes(g *gin.RouterGroup) { g.GET(\"/health\", health) }\n";
    let (a, b) = (main("/a"), main("/b"));
    assert_eq!(
        go_endpoint_uris(&[
            ("a/main.go", a.as_str()),
            ("a/routes.go", routes),
            ("b/main.go", b.as_str()),
            ("b/routes.go", routes),
        ]),
        vec!["/a/health", "/b/health"]
    );
}

#[test]
fn java_endpoints_only_get_own_class_prefix() {
    let controller = r#"
@RestController
@RequestMapping("/api")
public class ItemController {
    private static final String BASE = "/ignored";

    @GetMapping("/items")
    public String list() { return "x"; }

    @PostMapping("items/new")
    public String create() { return "x"; }
}
"#;
    let record = java_extract(controller, "svc/ItemController.java").expect("Java parses");
    // Pass 3 only joins the class prefix and drops the chain; other fields stay.
    let declared: Vec<_> = record
        .endpoints
        .iter()
        .zip(["/api/items", "/api/items/new"])
        .map(|(e, uri)| models::Endpoint {
            uri: uri.to_string(),
            route_prefixes: vec![],
            ..e.clone()
        })
        .collect();
    assert_eq!(declared.len(), 2);
    let other = router_file("BASE = \"/py\"", "BASE");
    let ir = evaluated(vec![record, py(&other, "py/app.py")], &HashMap::new());
    let java: Vec<_> = ir
        .endpoints
        .into_iter()
        .filter(|e| e.file_path == "svc/ItemController.java")
        .collect();
    assert_eq!(java, declared);
}

#[test]
fn imported_constant_wins_in_every_file_order() {
    let other = router_file("BASE_PATH = \"/other\"", "BASE_PATH");
    let config = "BASE_PATH = \"/mine\"\n";
    let routes = router_file("from svc.config import BASE_PATH", "BASE_PATH");
    let files = [
        ("other/routes.py", other.as_str()),
        ("svc/config.py", config),
        ("svc/routes.py", routes.as_str()),
    ];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let records = order.iter().map(|&i| py(files[i].1, files[i].0)).collect();
        assert_eq!(
            endpoint_uris(records),
            vec!["/mine/items", "/other/items"],
            "{order:?}"
        );
    }
}

/// Re-export of a re-export, against both file orders.
#[test]
fn reexported_prefix_constant_is_order_independent() {
    let root = "ROOT = \"/v1\"\n";
    let mid = "from root import ROOT\nMID = ROOT\n";
    let app = router_file("from mid import MID\nBASE = MID", "BASE");
    let uris = |order: [(&str, &str); 3]| {
        endpoint_uris(order.iter().map(|(path, code)| py(code, path)).collect())
    };
    let expected = vec!["/v1/items"];
    assert_eq!(
        uris([("root.py", root), ("mid.py", mid), ("app.py", &app)]),
        expected
    );
    assert_eq!(
        uris([("app.py", &app), ("mid.py", mid), ("root.py", root)]),
        expected
    );
}

/// A prefix that resolves for a REST call URL in the file resolves the same for an endpoint.
#[test]
fn go_group_const_from_other_file_matches_rest_call() {
    let constants = "package main\n\nconst API_BASE = \"/api\"\n";
    let main = r#"
package main

import "net/http"

func start() {
    e := echo.New()
    g := e.Group(API_BASE)
    g.POST("/carts", add)
}

func call() {
    url := API_BASE + "/x"
    http.Get(url)
}
"#;
    let ir = evaluated(
        vec![
            go_extract(constants, "svc/constants.go").expect("Go parses"),
            go_extract(main, "svc/main.go").expect("Go parses"),
        ],
        &HashMap::new(),
    );
    assert_eq!(ir.restcalls[0].target_uri, "/api/x");
    assert_eq!(ir.endpoints[0].uri, "/api/carts");
}

/// `BASE` is bound in neither the file nor its imports: two other files declare it.
#[test]
fn unbound_prefix_with_two_same_named_constants_is_order_independent() {
    let a = "BASE = \"/a\"\n";
    let b = "BASE = \"/b\"\n";
    let routes = router_file("", "BASE");
    let uris = |order: [(&str, &str); 3]| {
        endpoint_uris(order.iter().map(|(path, code)| py(code, path)).collect())
    };
    let declared = vec!["/items"];
    assert_eq!(
        uris([("a/c.py", a), ("b/c.py", b), ("svc/routes.py", &routes)]),
        declared
    );
    assert_eq!(
        uris([("b/c.py", b), ("a/c.py", a), ("svc/routes.py", &routes)]),
        declared
    );
    assert_eq!(
        uris([("svc/routes.py", &routes), ("b/c.py", b), ("a/c.py", a)]),
        declared
    );
}

/// Same value everywhere: not ambiguous, so the project constant applies.
#[test]
fn unbound_prefix_with_one_distinct_value_resolves() {
    let routes = router_file("", "BASE");
    assert_eq!(
        endpoint_uris(vec![
            py("BASE = \"/a\"\n", "a/c.py"),
            py("BASE = '/a'\n", "b/c.py"),
            py(&routes, "svc/routes.py"),
        ]),
        vec!["/a/items"]
    );
}

#[test]
fn second_module_round_independent_of_file_order() {
    let z = "BASE = \"/z\"\n";
    let a = "BASE = \"/a\"\n";
    let b = "from a import BASE\nD = BASE + \"/b\"\n";
    let c = router_file("from b import D\nE = D", "E");
    let uris = |order: [(&str, &str); 4]| {
        endpoint_uris(order.iter().map(|(path, code)| py(code, path)).collect())
    };
    assert_eq!(
        uris([("z.py", z), ("a.py", a), ("b.py", b), ("c.py", &c)]),
        uris([("z.py", z), ("a.py", a), ("c.py", &c), ("b.py", b)]),
    );
}

#[test]
fn lowercase_imported_constant_in_chain() {
    let routes = router_file("from config import root\nBASE = root", "BASE");
    assert_eq!(
        endpoint_uris(vec![
            py("root = \"/api\"\n", "config.py"),
            py(&routes, "routes.py"),
        ]),
        vec!["/api/items"]
    );
}

#[test]
fn external_constant_beats_imported_in_module_chain() {
    let external = HashMap::from([("root".to_string(), "/cli".to_string())]);
    let routes = router_file("from config import root\nBASE = root", "BASE");
    assert_eq!(
        uris_with(
            vec![
                py("root = \"/api\"\n", "config.py"),
                py(&routes, "routes.py"),
            ],
            &external
        ),
        vec!["/cli/items"]
    );
}

#[test]
fn quoted_concatenation_is_not_a_literal() {
    let code = router_file("X = \"/m\"", "\"/a\" + X + \"/b\"");
    assert_eq!(endpoint_uris(vec![py(&code, "svc/app.py")]), vec!["/items"]);
    let constant = router_file("X = \"/m\"\nP = \"/a\" + UNKNOWN + \"/b\"", "P");
    assert_eq!(
        endpoint_uris(vec![py(&constant, "svc/app.py")]),
        vec!["/items"]
    );
}

#[test]
fn own_value_not_poisoned_by_foreign_source_text() {
    let a = "V = f\"{Y}\"\n";
    let b = router_file("V = \"/x/{id}\"", "V");
    for records in [
        vec![py(a, "a/c.py"), py(&b, "b/routes.py")],
        vec![py(&b, "b/routes.py"), py(a, "a/c.py")],
    ] {
        assert_eq!(endpoint_uris(records), vec!["/x/{id}/items"]);
    }
}

#[test]
fn empty_constant_prefix_under_mount() {
    for (name, config) in [("X", "X = \"\""), ("x", "x = \"\"")] {
        let code = format!(
            r#"
from fastapi import FastAPI, APIRouter

{config}
app = FastAPI()
r = APIRouter(prefix={name})

@r.get("/x")
async def h():
    return None

app.include_router(r, prefix="/api")
"#
        );
        assert_eq!(
            endpoint_uris(vec![py(&code, "svc/app.py")]),
            vec!["/api/x"],
            "{name}"
        );
    }
}

#[test]
fn empty_imported_constant_prefix_under_mount() {
    let code = r#"
from fastapi import FastAPI, APIRouter
from config import x

app = FastAPI()
r = APIRouter(prefix=x)

@r.get("/x")
async def h():
    return None

app.include_router(r, prefix="/api")
"#;
    assert_eq!(
        endpoint_uris(vec![py("x = \"\"\n", "config.py"), py(code, "app.py")]),
        vec!["/api/x"]
    );
}

/// `is_route_registration` reads pass-2 URIs, which no longer carry the prefix.
#[test]
fn go_group_route_is_not_a_rest_call() {
    let code = r#"
package main

func start() {
    r := gin.Default()
    api := r.Group("/api")
    api.GET("/x", h)
}
"#;
    let ir = evaluated(
        vec![go_extract(code, "svc/main.go").expect("Go parses")],
        &HashMap::new(),
    );
    assert_eq!(ir.endpoints[0].uri, "/api/x");
    assert!(ir.restcalls.is_empty(), "{:?}", ir.restcalls);
}

#[test]
fn evaluated_endpoints_drop_chains() {
    let code = router_file("", "\"/api\"");
    let ir = evaluated(vec![py(&code, "svc/app.py")], &HashMap::new());
    assert!(ir.endpoints[0].route_prefixes.is_empty());
}

#[test]
fn chi_chained_route_closures_stay_separate() {
    let code = r#"
package main

import "github.com/go-chi/chi/v5"

func routes() {
    r := chi.NewRouter()
    r.Route("/a", func(r chi.Router) { r.Get("/x", h) }).Route("/b", func(r chi.Router) { r.Get("/y", h) })
}
"#;
    assert_eq!(
        go_endpoint_uris(&[("svc/main.go", code)]),
        vec!["/a/x", "/b/y"]
    );
}

#[test]
fn chi_earlier_closure_of_a_chain_keeps_outer_router() {
    let code = r#"
package main

import "github.com/go-chi/chi/v5"

func routes() {
    r := chi.NewRouter()
    r.Route("/a", func(x chi.Router) { r.Get("/z", h) }).Route("/b", func(r chi.Router) { r.Get("/y", h) })
}
"#;
    assert_eq!(
        go_endpoint_uris(&[("svc/main.go", code)]),
        vec!["/b/y", "/z"]
    );
}

#[test]
fn chi_closure_parameter_with_trailing_comma() {
    let code = r#"
package main

import "github.com/go-chi/chi/v5"

func routes() {
    r := chi.NewRouter()
    r.Route("/p", func(
        sub chi.Router,
    ) {
        sub.Get("/x", h)
    })
}
"#;
    assert_eq!(go_endpoint_uris(&[("svc/main.go", code)]), vec!["/p/x"]);
}

/// Django `urls.py` at `path` with `entries` in its `urlpatterns`.
fn urlconf(path: &str, entries: &[&str]) -> models::ir::syntax::FileRecord {
    py(
        &format!(
            "from django.urls import include, path, re_path\n\nurlpatterns = [\n{},\n]\n",
            entries.join(",\n")
        ),
        path,
    )
}

/// URIs of `path("x/", v)` in `app/urls.py` included by `proj/urls.py` through `entries`.
fn included_x(entries: &[&str]) -> Vec<String> {
    endpoint_uris(vec![
        urlconf("proj/urls.py", entries),
        urlconf("app/urls.py", &[r#"path("x/", v)"#]),
    ])
}

#[test]
fn django_include_by_module_string() {
    let uris = endpoint_uris(vec![
        urlconf(
            "svc/proj/urls.py",
            &[r#"path("api/", include("app.urls"))"#],
        ),
        urlconf("svc/app/urls.py", &[r#"path("items/", views.items)"#]),
    ]);
    assert_eq!(uris, vec!["/api/items/"]);
}

#[test]
fn django_include_module_is_package() {
    let uris = endpoint_uris(vec![
        urlconf(
            "svc/proj/urls.py",
            &[r#"path("api/", include("app.urls"))"#],
        ),
        urlconf(
            "svc/app/urls/__init__.py",
            &[r#"path("items/", views.items)"#],
        ),
    ]);
    assert_eq!(uris, vec!["/api/items/"]);
}

#[test]
fn django_include_tuple_form_and_namespace() {
    for entry in [
        r#"path("a/", include(("app.urls", "app")))"#,
        r#"path("a/", include("app.urls", namespace="x"))"#,
        r#"path("a/", include(("app.urls", "app"), namespace="x"), name="y")"#,
    ] {
        assert_eq!(included_x(&[entry]), vec!["/a/x/"], "{entry}");
    }
}

#[test]
fn django_include_nested() {
    let uris = endpoint_uris(vec![
        urlconf("proj/urls.py", &[r#"path("a/", include("app.urls"))"#]),
        urlconf("app/urls.py", &[r#"path("b/", include("sub.urls"))"#]),
        urlconf("sub/urls.py", &[r#"path("x/", v)"#]),
    ]);
    assert_eq!(uris, vec!["/a/b/x/"]);
}

#[test]
fn django_include_module_twice() {
    let uris = included_x(&[
        r#"path("v1/", include("app.urls"))"#,
        r#"path("v2/", include("app.urls"))"#,
    ]);
    assert_eq!(uris, vec!["/v1/x/", "/v2/x/"]);
}

#[test]
fn django_include_empty_prefix() {
    assert_eq!(
        included_x(&[r#"path("", include("app.urls"))"#]),
        vec!["x/"]
    );
}

#[test]
fn django_include_path_converter_in_prefix() {
    assert_eq!(
        included_x(&[r#"path("<int:org>/", include("app.urls"))"#]),
        vec!["/{org}/x/"]
    );
}

#[test]
fn django_include_prefix_from_constant_in_including_file() {
    let proj = r#"
from django.urls import include, path

API = "api/"

urlpatterns = [
    path(API, include("app.urls")),
]
"#;
    let uris = endpoint_uris(vec![
        py(proj, "proj/urls.py"),
        urlconf("app/urls.py", &[r#"path("x/", v)"#]),
    ]);
    assert_eq!(uris, vec!["/api/x/"]);
}

#[test]
fn django_include_plain_regex_prefix() {
    assert_eq!(
        included_x(&[r#"re_path(r"^api/", include("app.urls"))"#]),
        vec!["/api/x/"]
    );
}

#[test]
fn django_include_regex_prefix_with_metacharacters() {
    assert_eq!(
        included_x(&[r#"re_path(r"^v(?P<n>\d+)/", include("app.urls"))"#]),
        vec!["x/"]
    );
}

#[test]
fn django_include_view_methods_in_included_module() {
    let views = r#"
from rest_framework.decorators import api_view

@api_view(["GET", "POST"])
def view(request):
    pass
"#;
    let records = vec![
        urlconf("proj/urls.py", &[r#"path("api/", include("app.urls"))"#]),
        urlconf("app/urls.py", &[r#"path("x/", views.view)"#]),
        py(views, "app/views.py"),
    ];
    let endpoints = evaluated(records, &HashMap::new()).endpoints;
    assert_eq!(endpoints.len(), 2);
    for method in [HttpMethod::GET, HttpMethod::POST] {
        assert!(
            endpoints
                .iter()
                .any(|e| e.uri == "/api/x/" && e.http_method == method),
            "{endpoints:?}"
        );
    }
}

#[test]
fn django_include_module_outside_project() {
    let uris = endpoint_uris(vec![urlconf(
        "proj/urls.py",
        &[
            r#"path("health/", include("health_check.urls"))"#,
            r#"path("x/", v)"#,
        ],
    )]);
    assert_eq!(uris, vec!["x/"]);
}

#[test]
fn django_include_non_literal_module_argument() {
    let uris = endpoint_uris(vec![
        urlconf(
            "proj/urls.py",
            &[r#"path("api/", include(router.urls))"#, r#"path("x/", v)"#],
        ),
        urlconf("router/urls.py", &[r#"path("y/", v)"#]),
    ]);
    assert_eq!(uris, vec!["x/", "y/"]);
}

#[test]
fn django_include_including_file_is_not_its_own_module() {
    let uris = endpoint_uris(vec![
        urlconf("proj/api/urls.py", &[r#"path("v1/", include("api.urls"))"#]),
        urlconf("api/urls.py", &[r#"path("x/", v)"#]),
    ]);
    assert_eq!(uris, vec!["/v1/x/"]);
}

#[test]
fn django_include_circular() {
    let uris = endpoint_uris(vec![
        urlconf(
            "a/urls.py",
            &[r#"path("to_b/", include("b.urls"))"#, r#"path("x/", v)"#],
        ),
        urlconf(
            "b/urls.py",
            &[r#"path("to_a/", include("a.urls"))"#, r#"path("y/", v)"#],
        ),
    ]);
    assert_eq!(uris, vec!["x/", "y/"]);

    let uris = endpoint_uris(vec![
        urlconf("proj/urls.py", &[r#"path("r/", include("a.urls"))"#]),
        urlconf(
            "a/urls.py",
            &[r#"path("b/", include("b.urls"))"#, r#"path("x/", v)"#],
        ),
        urlconf(
            "b/urls.py",
            &[r#"path("a/", include("a.urls"))"#, r#"path("y/", v)"#],
        ),
    ]);
    assert_eq!(uris, vec!["/r/b/y/", "/r/x/"]);
}

/// Same constant name in both files: the prefix evaluates in the including file.
#[test]
fn django_include_prefix_constant_shadowed_in_included_file() {
    let proj = r#"
from django.urls import include, path

API = "api/"

urlpatterns = [
    path(API, include("app.urls")),
]
"#;
    let app = r#"
from django.urls import path

API = "wrong/"

urlpatterns = [
    path("x/", v),
]
"#;
    let uris = endpoint_uris(vec![py(proj, "proj/urls.py"), py(app, "app/urls.py")]);
    assert_eq!(uris, vec!["/api/x/"]);
}

/// Pipeline `(method, uri)` pairs for Java `(path, code)` files, sorted.
fn java_endpoints(files: &[(&str, &str)]) -> Vec<(String, String)> {
    let records = files
        .iter()
        .map(|(path, code)| java_extract(code, path).expect("Java parses"))
        .collect();
    let mut out: Vec<_> = evaluated(records, &HashMap::new())
        .endpoints
        .into_iter()
        .map(|e| (format!("{:?}", e.http_method), e.uri))
        .collect();
    out.sort();
    out
}

fn java_uris(files: &[(&str, &str)]) -> Vec<String> {
    let mut uris: Vec<_> = java_endpoints(files).into_iter().map(|(_, u)| u).collect();
    uris.sort();
    uris
}

/// One controller file `svc/C.java` with `class_annotations` and `body`.
fn java_class_uris(class_annotations: &str, body: &str) -> Vec<String> {
    let code = format!("{class_annotations}\npublic class C {{\n{body}\n}}\n");
    java_uris(&[("svc/C.java", &code)])
}

const GET_A: &str = r#"@GetMapping("/a") public String a() { return ""; }"#;
const GET_X: &str = r#"@GetMapping("/x") public String x() { return ""; }"#;

#[test]
fn java_jaxrs_path_with_produces() {
    assert_eq!(
        java_class_uris(
            r#"@Path("/users") @Produces(MediaType.APPLICATION_JSON)"#,
            r#"@GET @Path("/{id}") public String get(@PathParam("id") String id) { return ""; }"#,
        ),
        vec!["/users/{id}"]
    );
}

#[test]
fn java_mapping_then_annotation_with_value() {
    assert_eq!(
        java_class_uris(r#"@RequestMapping("/api") @CrossOrigin("*")"#, GET_A),
        vec!["/api/a"]
    );
}

#[test]
fn java_mapping_then_named_argument_annotation() {
    assert_eq!(
        java_class_uris(r#"@RequestMapping("/api/v1") @Api(tags = "x")"#, GET_A),
        vec!["/api/v1/a"]
    );
}

#[test]
fn java_two_classes_in_one_file() {
    let code = r#"
@RequestMapping("/one")
class A { @GetMapping("/a") public String a() { return ""; } }

@RequestMapping("/two")
class B { @GetMapping("/b") public String b() { return ""; } }
"#;
    assert_eq!(
        java_uris(&[("svc/AB.java", code)]),
        vec!["/one/a", "/two/b"]
    );
}

#[test]
fn java_nested_class_takes_own_prefix() {
    let code = r#"
@RequestMapping("/outer")
public class Outer {
    @RequestMapping("/inner")
    public static class Inner { @GetMapping("/x") public String x() { return ""; } }
}
"#;
    assert_eq!(java_uris(&[("svc/Outer.java", code)]), vec!["/inner/x"]);
}

#[test]
fn java_interface_with_mapping() {
    let code = r#"
@RequestMapping("/api")
public interface Api { @GetMapping("/x") String x(); }
"#;
    assert_eq!(java_uris(&[("svc/Api.java", code)]), vec!["/api/x"]);
}

#[test]
fn java_prefix_slashes_normalized() {
    assert_eq!(
        java_class_uris(
            r#"@RequestMapping("/api/")"#,
            r#"@GetMapping("x") public String x() { return ""; }"#,
        ),
        vec!["/api/x"]
    );
}

#[test]
fn java_no_class_prefix() {
    assert_eq!(java_class_uris("@RestController", GET_A), vec!["/a"]);
}

fn java_method_endpoints(annotation: &str) -> Vec<(String, String)> {
    let code =
        format!("public class C {{\n{annotation} public String a() {{ return \"\"; }}\n}}\n");
    java_endpoints(&[("svc/C.java", &code)])
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(m, u)| (m.to_string(), u.to_string()))
        .collect()
}

#[test]
fn java_request_mapping_single_method() {
    assert_eq!(
        java_method_endpoints(r#"@RequestMapping(value = "/a", method = RequestMethod.POST)"#),
        pairs(&[("POST", "/a")])
    );
}

#[test]
fn java_request_mapping_method_array() {
    assert_eq!(
        java_method_endpoints(
            r#"@RequestMapping(value = "/a", method = {RequestMethod.GET, RequestMethod.PUT})"#
        ),
        pairs(&[("GET", "/a"), ("PUT", "/a")])
    );
}

#[test]
fn java_request_mapping_no_method() {
    assert_eq!(
        java_method_endpoints(r#"@RequestMapping("/a")"#),
        pairs(&[("GET", "/a")])
    );
}

#[test]
fn java_feign_client_not_endpoint() {
    let code = r#"
@FeignClient("svc") @RequestMapping("/api")
public interface Client { @GetMapping("/x") String x(); }
"#;
    assert!(java_uris(&[("svc/Client.java", code)]).is_empty());
}

#[test]
fn java_class_prefix_array() {
    assert_eq!(
        java_class_uris(r#"@RequestMapping({"/a", "/b"})"#, GET_X),
        vec!["/a/x", "/b/x"]
    );
}

#[test]
fn java_method_path_array() {
    assert_eq!(
        java_class_uris(
            "",
            r#"@GetMapping({"/x", "/y"}) public String x() { return ""; }"#
        ),
        vec!["/x", "/y"]
    );
}

#[test]
fn java_class_and_method_arrays() {
    assert_eq!(
        java_class_uris(
            r#"@RequestMapping({"/a", "/b"})"#,
            r#"@GetMapping({"/x", "/y"}) public String x() { return ""; }"#
        ),
        vec!["/a/x", "/a/y", "/b/x", "/b/y"]
    );
}

#[test]
fn java_qualified_constant_prefix() {
    let paths = r#"public class Paths { public static final String BASE = "/api"; }"#;
    let controller = format!("@RequestMapping(Paths.BASE)\npublic class C {{\n{GET_X}\n}}\n");
    assert_eq!(
        java_uris(&[("svc/Paths.java", paths), ("svc/C.java", &controller)]),
        vec!["/api/x"]
    );
}

#[test]
fn java_same_class_constant_prefix() {
    assert_eq!(
        java_class_uris(
            "@RequestMapping(BASE)",
            &format!("static final String BASE = \"/api\";\n{GET_X}")
        ),
        vec!["/api/x"]
    );
}

#[test]
fn java_unresolved_constant_prefix_dropped() {
    assert_eq!(
        java_class_uris("@RequestMapping(Config.base)", GET_X),
        vec!["/x"]
    );
}

#[test]
fn java_method_level_constant_unchanged() {
    assert_eq!(
        java_class_uris(
            "",
            r#"@GetMapping(Paths.ITEM) public String x() { return ""; }"#
        ),
        vec!["Paths.ITEM"]
    );
}

#[test]
fn java_and_python_in_one_project() {
    let fastapi = r#"
from fastapi import FastAPI, APIRouter

app = FastAPI()
r = APIRouter()

@r.get("/q")
async def q():
    return None

app.include_router(r, prefix="/p")
"#;
    let controller = format!("@RequestMapping(\"/api\")\npublic class C {{\n{GET_X}\n}}\n");
    let records = vec![
        py(fastapi, "py/app.py"),
        java_extract(&controller, "svc/C.java").expect("Java parses"),
    ];
    assert_eq!(endpoint_uris(records), vec!["/api/x", "/p/q"]);
}

#[test]
fn java_class_prefix_with_block_comment() {
    assert_eq!(
        java_class_uris(r#"@RequestMapping(/* v1 */ "/api")"#, GET_X),
        vec!["/api/x"]
    );
}

#[test]
fn java_class_prefix_with_line_comment() {
    assert_eq!(
        java_class_uris("@RequestMapping(\n    \"/api\" // v1\n)", GET_X),
        vec!["/api/x"]
    );
}

#[test]
fn java_anonymous_class_method_takes_no_prefix() {
    let body = r#"
    @GetMapping("/x") public String x() { return ""; }
    Object h = new Object() {
        @GetMapping("/y") public String y() { return ""; }
    };
"#;
    assert_eq!(
        java_class_uris(r#"@RequestMapping("/api")"#, body),
        vec!["/api/x", "/y"]
    );
}

#[test]
fn java_class_prefix_array_capped() {
    let paths: Vec<String> = (0..300).map(|i| format!("\"/p{i}\"")).collect();
    let uris = java_class_uris(&format!("@RequestMapping({{{}}})", paths.join(", ")), GET_X);
    assert_eq!(uris.len(), 256);
    assert!(uris.contains(&"/p0/x".to_string()));
    assert!(!uris.contains(&"/p299/x".to_string()));
}

#[test]
fn java_concatenated_class_prefix_dropped() {
    assert_eq!(
        java_class_uris(r#"@RequestMapping(Paths.BASE + "/users")"#, GET_X),
        vec!["/x"]
    );
}
