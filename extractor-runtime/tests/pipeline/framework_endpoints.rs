use extractor_runtime::pipeline;
use go_extractor::extraction::extract_syntactic as go_extract;
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

fn endpoint_uris(records: Vec<models::ir::syntax::FileRecord>) -> Vec<String> {
    let project_ir = pipeline::build_project_ir(records);
    let mut uris: Vec<String> = project_ir
        .files
        .iter()
        .flat_map(|file| file.endpoints.iter().map(|e| e.uri.clone()))
        .collect();
    uris.sort();
    uris
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
