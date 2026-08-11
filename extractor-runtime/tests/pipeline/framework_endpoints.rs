use extractor_runtime::pipeline;
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

@api_view(["GET"])
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
