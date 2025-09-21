use python_extractor::{
    extraction::{
        assignments::map::get_assignments_map,
        extractor::{ExtractParams, Extractor},
        restcalls::{evaluator::evaluate_restcalls, extractor::RestcallsExtractor},
    },
    s,
    utils::load_file,
};

use models::{Argument, HttpMethod, RestCall};

use crate::python::utils::get_tree;

#[test]
fn restcalls_extraction() {
    let filename = "./examples/python/restcalls.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let restcalls = RestcallsExtractor
        .extract(ExtractParams::new(&tree, &code).service_name(&s!("test_service")));
    let expected = vec![
        RestCall {
            function_name: s!("create_item"),
            function_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("payload"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("{BASE_URL}/items/"),
            service_name: s!("test_service"),
        },
        RestCall {
            function_name: s!("get_items"),
            function_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("params"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/items/"),
            service_name: s!("test_service"),
        },
        RestCall {
            function_name: s!("get_item_by_id"),
            function_arguments: vec![],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/items/{item_id}"),
            service_name: s!("test_service"),
        },
        RestCall {
            function_name: s!("create_user"),
            function_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("payload"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("{BASE_URL}/users/"),
            service_name: s!("test_service"),
        },
        RestCall {
            function_name: s!("get_users"),
            function_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"limit\": limit}"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/users/"),
            service_name: s!("test_service"),
        },
        RestCall {
            function_name: s!("search"),
            function_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"q\": query}"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/search/"),
            service_name: s!("test_service"),
        },
    ];

    assert_eq!(restcalls, expected);
}

#[test]
fn restcalls_evaluation() {
    let filename = "./examples/python/restcalls.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let mut restcalls = RestcallsExtractor
        .extract(ExtractParams::new(&tree, &code).service_name(&s!("test_service")));
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_restcalls(&mut restcalls, assignments_map);
    let expected = vec![
        RestCall {
            function_name: s!("create_item"),
            function_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!(
                    "{\n        \"name\": name,\n        \"description\": description,\n        \"price\": price,\n        \"in_stock\": in_stock\n    }"
                ),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/items/"),
            service_name: s!("test_service"),
        },
        RestCall {
            function_name: s!("get_items"),
            function_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"skip\": skip, \"limit\": limit}"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/items/"),
            service_name: s!("test_service"),
        },
        RestCall {
            function_name: s!("get_item_by_id"),
            function_arguments: vec![],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/items/{item_id}"),
            service_name: s!("test_service"),
        },
        RestCall {
            function_name: s!("create_user"),
            function_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("{\n        \"username\": username,\n        \"email\": email\n    }"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/users/"),
            service_name: s!("test_service"),
        },
        RestCall {
            function_name: s!("get_users"),
            function_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"limit\": limit}"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/users/"),
            service_name: s!("test_service"),
        },
        RestCall {
            function_name: s!("search"),
            function_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"q\": query}"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/search/"),
            service_name: s!("test_service"),
        },
    ];

    assert_eq!(restcalls, expected);
}
