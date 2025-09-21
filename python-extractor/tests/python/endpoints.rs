use python_extractor::{
    extraction::{
        endpoints::extractor::EndpointsExtractor,
        extractor::{ExtractParams, Extractor},
    },
    s, strs,
    utils::load_file,
};

use crate::python::utils::get_tree;

use models::{Endpoint, HttpMethod};

#[test]
fn base_test() {
    let filename = "./examples/python/endpoints.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let endpoints = EndpointsExtractor
        .extract(ExtractParams::new(&tree, &code).service_name(&s!("test_service")));
    let expected = vec![
        Endpoint {
            function_name: s!("create_item"),
            http_method: HttpMethod::POST,
            parameters: vec![],
            uri: s!("/items/"),
            service_name: s!("test_service"),
        },
        Endpoint {
            function_name: s!("read_items"),
            http_method: HttpMethod::GET,
            parameters: strs!["skip", "limit", "q"],
            uri: s!("/items/"),
            service_name: s!("test_service"),
        },
        Endpoint {
            function_name: s!("read_item"),
            http_method: HttpMethod::GET,
            parameters: strs!["item_id"],
            uri: s!("/items/{item_id}"),
            service_name: s!("test_service"),
        },
        Endpoint {
            function_name: s!("update_item"),
            http_method: HttpMethod::PUT,
            parameters: vec![],
            uri: s!("/items/{item_id}"),
            service_name: s!("test_service"),
        },
        Endpoint {
            function_name: s!("delete_item"),
            http_method: HttpMethod::DELETE,
            parameters: vec![],
            uri: s!("/items/{item_id}"),
            service_name: s!("test_service"),
        },
        Endpoint {
            function_name: s!("create_user"),
            http_method: HttpMethod::POST,
            parameters: vec![],
            uri: s!("/users/"),
            service_name: s!("test_service"),
        },
        Endpoint {
            function_name: s!("list_users"),
            http_method: HttpMethod::GET,
            parameters: strs!["limit"],
            uri: s!("/users/"),
            service_name: s!("test_service"),
        },
        Endpoint {
            function_name: s!("get_user"),
            http_method: HttpMethod::GET,
            parameters: strs!["user_id"],
            uri: s!("/users/{user_id}"),
            service_name: s!("test_service"),
        },
        Endpoint {
            function_name: s!("search"),
            http_method: HttpMethod::GET,
            parameters: strs!["q"],
            uri: s!("/search/"),
            service_name: s!("test_service"),
        },
    ];

    assert_eq!(endpoints, expected);
}
