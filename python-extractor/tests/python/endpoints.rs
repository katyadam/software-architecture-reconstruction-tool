use python_extractor::{
    extraction::{
        endpoints::{EndpointStrategy, PythonEndpointStrategy, extractor::EndpointsExtractor},
        extractor::{ExtractParams, Extractor},
    },
    s,
};

use crate::python::utils::{get_tree, load_file, parse_file};

use models::{Endpoint, HttpMethod, Parameter};
use models::ir::{language::Language, project::TypedFileRecord, syntax::FileRecord};

#[test]
fn base_test() {
    let filename = "./examples/python/endpoints.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let endpoints =
        EndpointsExtractor.extract(ExtractParams::new(&tree, &code).file_name(&s!(filename)));
    let expected = vec![
        Endpoint {
            function_name: s!("create_item"),
            function_hash: s!("912172a93ead73ec76396b25cca3afc21232de1fd4bf9611f0cbedca05089c17"),

            http_method: HttpMethod::POST,
            parameters: vec![Parameter {
                name: s!("item"),
                datatype: Some(s!("ItemCreate")),
                initial_value: None,
            }],
            uri: s!("/items/"),
            file_path: s!(filename),
            router_variable: Some(s!("app")),
        },
        Endpoint {
            function_name: s!("read_items"),
            function_hash: s!("f445c22afc5b9172b23ecdcedb09ca43ffdcede95a60fb68bc2604213dbd0c03"),

            http_method: HttpMethod::GET,
            parameters: vec![
                Parameter {
                    name: s!("skip"),
                    datatype: Some(s!("int")),
                    initial_value: Some(s!("0")),
                },
                Parameter {
                    name: s!("limit"),
                    datatype: Some(s!("int")),
                    initial_value: Some(s!("10")),
                },
                Parameter {
                    name: s!("q"),
                    datatype: Some(s!("Optional[str]")),
                    initial_value: Some(s!("None")),
                },
            ],
            uri: s!("/items/"),
            file_path: s!(filename),
            router_variable: Some(s!("app")),
        },
        Endpoint {
            function_name: s!("read_item"),
            function_hash: s!("ab530894592d99a20d5d585d8be1c9eb9a807515f767af6d1f10a1206d946344"),

            http_method: HttpMethod::GET,
            parameters: vec![Parameter {
                name: s!("item_id"),
                datatype: Some(s!("int")),
                initial_value: Some(s!("0")),
            }],
            uri: s!("/items/{item_id}"),
            file_path: s!(filename),
            router_variable: Some(s!("app")),
        },
        Endpoint {
            function_name: s!("update_item"),
            function_hash: s!("681b904b7f0665b530b3ff7b89447dadf5eda0e691002554d091f5573dce8cfe"),

            http_method: HttpMethod::PUT,
            parameters: vec![
                Parameter {
                    name: s!("item_id"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("item"),
                    datatype: Some(s!("ItemCreate")),
                    initial_value: None,
                },
            ],
            uri: s!("/items/{item_id}"),
            file_path: s!(filename),
            router_variable: Some(s!("app")),
        },
        Endpoint {
            function_name: s!("delete_item"),
            function_hash: s!("8a27fce2f6d0db05b270f395b4ccb89ca719bfef4a303a3e84de7e53b3623102"),
            http_method: HttpMethod::DELETE,
            parameters: vec![Parameter {
                name: s!("item_id"),
                datatype: Some(s!("int")),
                initial_value: None,
            }],
            uri: s!("/items/{item_id}"),
            file_path: s!(filename),
            router_variable: Some(s!("app")),
        },
        Endpoint {
            function_name: s!("create_user"),
            function_hash: s!("79b0ed224f38ee0df39dd127b2f81a5bff7152732020a8a6acd0826f90b15f45"),
            http_method: HttpMethod::POST,
            parameters: vec![Parameter {
                name: s!("user"),
                datatype: Some(s!("UserCreate")),
                initial_value: None,
            }],
            uri: s!("/users/"),
            file_path: s!(filename),
            router_variable: Some(s!("app")),
        },
        Endpoint {
            function_name: s!("list_users"),
            function_hash: s!("1f916bd0c0c210530c7212edf9be5116524eb55c89b625f02d7936f80a6996c4"),
            http_method: HttpMethod::GET,
            parameters: vec![Parameter {
                name: s!("limit"),
                datatype: Some(s!("int")),
                initial_value: Some(s!("Query(10, le=100)")),
            }],
            uri: s!("/users/"),
            file_path: s!(filename),
            router_variable: Some(s!("app")),
        },
        Endpoint {
            function_name: s!("get_user"),
            function_hash: s!("3801fe5bc3ecc991564ec370d99d10e2e7572f9f0acb913578f8adf8eb6fdf03"),
            http_method: HttpMethod::GET,
            parameters: vec![Parameter {
                name: s!("user_id"),
                datatype: Some(s!("int")),
                initial_value: Some(s!("Path(..., gt=0)")),
            }],
            uri: s!("/users/{user_id}"),
            file_path: s!(filename),
            router_variable: Some(s!("app")),
        },
        Endpoint {
            function_name: s!("search"),
            function_hash: s!("cb2c122a40999ee1050c12b009f41092b35878a02a010b397c03720a69a517c6"),
            http_method: HttpMethod::GET,
            parameters: vec![Parameter {
                name: s!("q"),
                datatype: Some(s!("str")),
                initial_value: Some(s!("Query(..., min_length=2)")),
            }],
            uri: s!("/search/"),
            file_path: s!(filename),
            router_variable: Some(s!("app")),
        },
    ];

    assert_eq!(endpoints, expected);
}

#[test]
fn python_endpoint_strategy_handles_framework_routes() {
    let code = r#"
from fastapi import FastAPI
from flask import Flask

api = FastAPI()
app = Flask(__name__)

@api.post("/items")
def create_item():
    pass

@app.route("/orders", methods=["GET", "PATCH"])
def update_order():
    pass
"#;
    let tree = get_tree(code);
    let endpoints = EndpointStrategy::extract(
        &PythonEndpointStrategy,
        ExtractParams::new(&tree, code).file_name("frameworks.py"),
    );

    assert!(endpoints.iter().any(|endpoint| {
        endpoint.function_name == "create_item" && endpoint.http_method == HttpMethod::POST
    }));
    assert!(endpoints.iter().any(|endpoint| {
        endpoint.function_name == "update_order" && endpoint.http_method == HttpMethod::GET
    }));
    assert!(endpoints.iter().any(|endpoint| {
        endpoint.function_name == "update_order" && endpoint.http_method == HttpMethod::PATCH
    }));
}

#[test]
fn test_endpoint_edge_cases() {
    let filename = "./examples/python/endpoint_edge_cases.py";
    let (code, tree) = parse_file(filename);
    let endpoints =
        EndpointsExtractor.extract(ExtractParams::new(&tree, &code).file_name(&s!(filename)));

    assert_eq!(
        endpoints.len(),
        3,
        "Expected 3 endpoints: PATCH, GET (list), GET (search)"
    );

    // Edge case 1: @app.patch produces a PATCH endpoint — not present in existing tests
    let patch_endpoints: Vec<&Endpoint> = endpoints
        .iter()
        .filter(|e| e.http_method == HttpMethod::PATCH)
        .collect();
    assert_eq!(
        patch_endpoints.len(),
        1,
        "Expected exactly 1 PATCH endpoint"
    );
    assert_eq!(patch_endpoints[0].function_name, s!("update_item_partial"));
    assert_eq!(patch_endpoints[0].uri, s!("/items/{item_id}"));

    // Edge case 2: Parameters with plain numeric defaults — verify standard default capture
    let list_ep = endpoints
        .iter()
        .find(|e| e.function_name == "list_items")
        .unwrap();
    assert_eq!(list_ep.http_method, HttpMethod::GET);
    assert_eq!(list_ep.parameters.len(), 2);
    assert_eq!(list_ep.parameters[0].name, s!("skip"));
    assert_eq!(list_ep.parameters[0].initial_value, Some(s!("0")));
    assert_eq!(list_ep.parameters[0].datatype, Some(s!("int")));
    assert_eq!(list_ep.parameters[1].name, s!("limit"));
    assert_eq!(list_ep.parameters[1].initial_value, Some(s!("10")));

    // Edge case 3: Optional[str] parameter with None default — the full Optional[str]
    // type is preserved in datatype, and "None" is the initial_value
    let search_ep = endpoints
        .iter()
        .find(|e| e.function_name == "search_items")
        .unwrap();
    assert_eq!(search_ep.http_method, HttpMethod::GET);
    assert_eq!(search_ep.uri, s!("/search/"));
    assert_eq!(search_ep.parameters.len(), 1);
    assert_eq!(search_ep.parameters[0].name, s!("q"));
    assert_eq!(search_ep.parameters[0].datatype, Some(s!("Optional[str]")));
    assert_eq!(search_ep.parameters[0].initial_value, Some(s!("None")));
}

#[test]
fn extracts_fastapi_flask_and_django_endpoint_styles() {
    let filename = "framework_endpoints.py";
    let code = r#"
from flask import Flask
from fastapi import FastAPI
from django.urls import path

app = FastAPI()
flask_app = Flask(__name__)

@app.get("/items/{item_id}")
def fastapi_get_item(item_id: int):
    pass

@flask_app.route("/orders/<int:order_id>", methods=["GET", "POST"])
def flask_order(order_id):
    pass

@flask_app.route("/health")
def flask_health():
    pass

urlpatterns = [
    path("products/", views.get_products),
    path("products/id=<int:id>/", views.get_product),
    path("ignored/", include("nested.urls")),
]
"#;
    let tree = get_tree(code);
    let endpoints = EndpointsExtractor.extract(ExtractParams::new(&tree, code).file_name(filename));

    assert!(endpoints.iter().any(|endpoint| {
        endpoint.function_name == "fastapi_get_item"
            && endpoint.http_method == HttpMethod::GET
            && endpoint.uri == "/items/{item_id}"
            && endpoint.router_variable.as_deref() == Some("app")
    }));
    assert!(endpoints.iter().any(|endpoint| {
        endpoint.function_name == "flask_order"
            && endpoint.http_method == HttpMethod::GET
            && endpoint.uri == "/orders/{order_id}"
            && endpoint.router_variable.as_deref() == Some("flask_app")
    }));
    assert!(endpoints.iter().any(|endpoint| {
        endpoint.function_name == "flask_order"
            && endpoint.http_method == HttpMethod::POST
            && endpoint.uri == "/orders/{order_id}"
    }));
    assert!(endpoints.iter().any(|endpoint| {
        endpoint.function_name == "flask_health"
            && endpoint.http_method == HttpMethod::GET
            && endpoint.uri == "/health"
    }));
    assert!(endpoints.iter().any(|endpoint| {
        endpoint.function_name == "get_products"
            && endpoint.http_method == HttpMethod::GET
            && endpoint.uri == "products/"
            && endpoint.router_variable.as_deref() == Some("urlpatterns")
    }));
    assert!(endpoints.iter().any(|endpoint| {
        endpoint.function_name == "get_product" && endpoint.uri == "products/id={id}/"
    }));
    assert!(endpoints.iter().all(|endpoint| endpoint.uri != "ignored/"));
}

#[test]
fn python_post_process_skips_non_python_files() {
    let mut files = vec![
        TypedFileRecord::from(FileRecord {
            file_path: "controller.java".to_string(),
            language: Language::Java,
            imports: vec![],
            entities: vec![],
            endpoints: vec![Endpoint {
                function_name: "status".to_string(),
                function_hash: "java-hash".to_string(),
                http_method: HttpMethod::GET,
                parameters: vec![],
                uri: "/status".to_string(),
                file_path: "controller.java".to_string(),
                router_variable: Some("controller".to_string()),
            }],
            callables: vec![],
            call_statements: vec![],
            assignments: std::collections::HashMap::new(),
            enums: vec![],
        }),
        TypedFileRecord::from(FileRecord {
            file_path: "views.py".to_string(),
            language: Language::Python,
            imports: vec![],
            entities: vec![],
            endpoints: vec![
                Endpoint {
                    function_name: "products".to_string(),
                    function_hash: "placeholder".to_string(),
                    http_method: HttpMethod::POST,
                    parameters: vec![],
                    uri: String::new(),
                    file_path: "views.py".to_string(),
                    router_variable: None,
                },
                Endpoint {
                    function_name: "products".to_string(),
                    function_hash: String::new(),
                    http_method: HttpMethod::GET,
                    parameters: vec![],
                    uri: "products/".to_string(),
                    file_path: "views.py".to_string(),
                    router_variable: Some("urlpatterns".to_string()),
                },
            ],
            callables: vec![],
            call_statements: vec![],
            assignments: std::collections::HashMap::new(),
            enums: vec![],
        }),
    ];

    let mut python_files = files
        .iter_mut()
        .filter(|file| file.language == Language::Python)
        .collect::<Vec<_>>();
    python_extractor::extraction::post_process(&mut python_files);

    assert_eq!(files[0].endpoints.len(), 1);
    assert_eq!(files[0].endpoints[0].uri, "/status");
    assert_eq!(files[0].endpoints[0].function_name, "status");

    assert_eq!(files[1].endpoints.len(), 1);
    assert_eq!(files[1].endpoints[0].http_method, HttpMethod::POST);
    assert_eq!(files[1].endpoints[0].uri, "products/");
}
