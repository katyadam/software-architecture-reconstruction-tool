use std::collections::HashMap;

use python_extractor::{
    extraction::{
        assignments::map::get_assignments_map,
        calls::extractor::CallsExtractor,
        enums::map::get_enums_map,
        extractor::{ExtractParams, Extractor},
        restcalls::{
            evaluation::{
                assignment::evaluate_local_and_global_assignments,
                method_call::MethodCallEvaluationStrategy,
            },
            identification::method_call::MethodCallIdentificationStrategy,
            selection::{method_call::MethodCallSelector, selector::Selector},
        },
    },
    s,
};

use models::{Argument, HttpMethod, RestCall, enums::Enum};
use statix::parse_python;
use tree_sitter::Tree;

use crate::python::utils::{get_tree, load_file, parse_file};

fn restcalls(code: &str, tree: &Tree, file_name: &str) -> Vec<RestCall> {
    let calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));
    let function_asts = parse_python(&tree, &code);
    MethodCallSelector::new(
        MethodCallIdentificationStrategy::new(),
        MethodCallEvaluationStrategy::new(function_asts, HashMap::new()),
    )
    .select_restcall_statements(&calls, file_name)
    .expect("This test should not fail!")
}

#[test]
fn restcalls_extraction() {
    let filename = "./examples/python/restcalls/large_example.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);

    let restcalls = restcalls(&code, &tree, &filename);
    let expected = vec![
        RestCall {
            function_name: s!(
                "create_item(client, name, description, price, in_stock=True) -> Any"
            ),
            function_hash: s!("afa02c3cc6ad3266bdb082ee5fd59da7bf9dfe5dfdffab62dcd15e63767190d7"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/items/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("json"),
                    value: s!("payload"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::POST,
            target_uri: s!("{BASE_URL}/items/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_items(client, skip=0, limit=10, q=None) -> Any"),
            function_hash: s!("055042d0e729e4f644a5248202166c09d27303f290b5ea9bc454054a85a5ca37"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/items/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("params"),
                    value: s!("params"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/items/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_item_by_id(client, item_id) -> Any"),
            function_hash: s!("520b826947096ff0d74f8c220d0e5eef242674e1a1703dd555beaaa31c1eb005"),
            call_arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("f\"{BASE_URL}/items/{item_id}\""),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/items/{item_id}"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("create_user(client, username, email) -> Any"),
            function_hash: s!("28c43beff05e39512caad64915e9712bcd46971e0ed481a7eded80800682d323"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/users/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("json"),
                    value: s!("payload"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::POST,
            target_uri: s!("{BASE_URL}/users/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_users(client, limit=10) -> Any"),
            function_hash: s!("c104b36cd276fe55ee93ddc980be3571136d7578d929aa4dc06bd6c0cec46b29"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/users/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("params"),
                    value: s!("{\"limit\": limit}"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/users/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("search(client, query) -> Any"),
            function_hash: s!("6212962b8d696d80dc03f12a1ddc465e85103d47b3bdce515d29658def10bb49"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/search/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("params"),
                    value: s!("{\"q\": query}"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/search/"),
            file_path: s!(filename),
        },
    ];

    assert_eq!(restcalls, expected);
}

#[test]
fn restcalls_evaluation() {
    let filename = "./examples/python/restcalls/large_example.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let mut restcalls = restcalls(&code, &tree, &filename);
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_local_and_global_assignments(&mut restcalls, &assignments_map);
    let expected = vec![
        RestCall {
            function_name: s!(
                "create_item(client, name, description, price, in_stock=True) -> Any"
            ),
            function_hash: s!("afa02c3cc6ad3266bdb082ee5fd59da7bf9dfe5dfdffab62dcd15e63767190d7"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/items/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("json"),
                    value: s!(
                        "{\n        \"name\": name,\n        \"description\": description,\n        \"price\": price,\n        \"in_stock\": in_stock\n    }"
                    ),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_items(client, skip=0, limit=10, q=None) -> Any"),
            function_hash: s!("055042d0e729e4f644a5248202166c09d27303f290b5ea9bc454054a85a5ca37"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/items/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("params"),
                    value: s!("{\"skip\": skip, \"limit\": limit}"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_item_by_id(client, item_id) -> Any"),

            function_hash: s!("520b826947096ff0d74f8c220d0e5eef242674e1a1703dd555beaaa31c1eb005"),
            call_arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("f\"{BASE_URL}/items/{item_id}\""),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/items/{item_id}"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("create_user(client, username, email) -> Any"),

            function_hash: s!("28c43beff05e39512caad64915e9712bcd46971e0ed481a7eded80800682d323"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/users/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("json"),
                    value: s!(
                        "{\n        \"username\": username,\n        \"email\": email\n    }"
                    ),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_users(client, limit=10) -> Any"),
            function_hash: s!("c104b36cd276fe55ee93ddc980be3571136d7578d929aa4dc06bd6c0cec46b29"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/users/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("params"),
                    value: s!("{\"limit\": limit}"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("search(client, query) -> Any"),
            function_hash: s!("6212962b8d696d80dc03f12a1ddc465e85103d47b3bdce515d29658def10bb49"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/search/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("params"),
                    value: s!("{\"q\": query}"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/search/"),
            file_path: s!(filename),
        },
    ];

    assert_eq!(restcalls, expected);
}

#[test]
fn should_extract_all_types_of_restcall() {
    let filename = "./examples/python/restcalls/different_types.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let mut restcalls = restcalls(&code, &tree, &filename);
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_local_and_global_assignments(&mut restcalls, &assignments_map);
    let expected = vec![
        RestCall {
            function_name: s!("endpoint_with_withblock_restcall(data: ProxyItemCreate) -> Any"),
            function_hash: s!("fc2daf1e0f6ec57bb37709b801791367a3cfd6065a0e298cb9ac45905a7e98d1"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/items/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("json"),
                    value: s!("data.dict()"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("withblock_restcall_assignment(data: ProxyItemCreate) -> Any"),
            function_hash: s!("15afbd16c19e74e78c29dcda1baa94ff0b40d33a4c4721ee050372d6a1d96258"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/items/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("json"),
                    value: s!("data.dict()"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("restcall_assignment(client, skip=0, limit=10, q=None) -> Any"),
            function_hash: s!("ff31910e5c469e50be506cff25ba0327329290a86de5b8c88c135de8824f905e"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/items/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("params"),
                    value: s!("{\"skip\": skip, \"limit\": limit}"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("restcall_no_assignment(client, username, email) -> Any"),
            function_hash: s!("a6c710dded34b961089f5a268302fb0448d6dca9ac87ebb8e7892fa5633a4f95"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/users/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("json"),
                    value: s!(
                        "{\n        \"username\": username,\n        \"email\": email\n    }"
                    ),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("await_restcall_assignment(client, username, email) -> Any"),
            function_hash: s!("fe73f93925fc417362d5b550090167b84c9175a17c4f5d80a0f86fcb6fe7473f"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/users/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("json"),
                    value: s!(
                        "{\n        \"username\": username,\n        \"email\": email\n    }"
                    ),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("await_restcall_no_assignment(client, username, email) -> Any"),
            function_hash: s!("31d2676275664125e30230a31dabbc0c7cb77d02b5a160e170b646b36b126e4b"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("f\"{BASE_URL}/users/\""),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("json"),
                    value: s!(
                        "{\n        \"username\": username,\n        \"email\": email\n    }"
                    ),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
    ];

    assert_eq!(restcalls, expected);
}

#[test]
fn should_assign_correct_target_uris_using_symbolic_evaluation() {
    let filename = "./examples/python/restcalls/url_not_in_call.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let mut restcalls = restcalls(&code, &tree, &filename);
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_local_and_global_assignments(&mut restcalls, &assignments_map);

    let expected = vec![RestCall {
        function_name: s!(
            "create_item(client, name: str, description, price, in_stock=True) -> str"
        ),
        function_hash: s!("ebc264ec787b0fcb8af627995a92933744423203b8fa7a074fb0f20d9691d1eb"),
        call_arguments: vec![
            Argument {
                assigned_variable: s!(""),
                value: s!("f\"{BASE_URL}/items/\""),
                datatype: s!("any"),
            },
            Argument {
                assigned_variable: s!("json"),
                value: s!(
                    "{\n        \"name\": name,\n        \"description\": description,\n        \"price\": price,\n        \"in_stock\": in_stock\n    }"
                ),
                datatype: s!("any"),
            },
        ],
        http_method: HttpMethod::POST,
        target_uri: s!("http://abrakadabra/items/"),
        file_path: s!("./examples/python/restcalls/url_not_in_call.py"),
    }];

    assert_eq!(restcalls, expected);
}

#[test]
fn should_not_identify_endpoints_with_restcall_notation() {
    let filename = "./examples/python/restcalls/endpoints_with_restcall_notation.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let mut restcalls = restcalls(&code, &tree, &filename);
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_local_and_global_assignments(&mut restcalls, &assignments_map);

    assert!(restcalls.is_empty());
}

#[test]
fn should_correctly_generate_multiple_restcalls_with_resolved_enums() {
    let filename = "./examples/python/restcalls/enum_in_restcall_uri.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));
    let function_asts = parse_python(&tree, &code);
    let mut restcalls = MethodCallSelector::new(
        MethodCallIdentificationStrategy::new(),
        MethodCallEvaluationStrategy::new(
            function_asts,
            get_enums_map(&vec![Enum {
                name: s!("MappingType"),
                values: vec![s!("cases"), s!("slides")],
            }]),
        ),
    )
    .select_restcall_statements(&calls, filename)
    .expect("This test should not fail!");

    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_local_and_global_assignments(&mut restcalls, &assignments_map);

    assert_eq!(restcalls.len(), 4, "expected 4 REST calls (2 enum values × 2 functions)");

    // `idms_url` is imported (not an assignment), so it stays unresolved in the URI.
    // Each enum value must appear exactly once per function.
    let get_calls: Vec<&RestCall> = restcalls
        .iter()
        .filter(|r| r.http_method == HttpMethod::GET)
        .collect();
    let put_calls: Vec<&RestCall> = restcalls
        .iter()
        .filter(|r| r.http_method == HttpMethod::PUT)
        .collect();

    assert_eq!(get_calls.len(), 2, "fetch_mapping uses GET — expect one call per enum value");
    assert_eq!(put_calls.len(), 2, "fetch_mappings uses PUT — expect one call per enum value");

    // Both enum values must appear in GET call URIs (fetch_mapping)
    let get_uris: Vec<&str> = get_calls.iter().map(|r| r.target_uri.as_str()).collect();
    assert!(
        get_uris.iter().any(|u| u.contains("/cases/")),
        "GET calls should include a URI with /cases/, got: {get_uris:?}"
    );
    assert!(
        get_uris.iter().any(|u| u.contains("/slides/")),
        "GET calls should include a URI with /slides/, got: {get_uris:?}"
    );

    // Both enum values must appear in PUT call URIs (fetch_mappings)
    let put_uris: Vec<&str> = put_calls.iter().map(|r| r.target_uri.as_str()).collect();
    assert!(
        put_uris.iter().any(|u| u.contains("/cases/")),
        "PUT calls should include a URI with /cases/, got: {put_uris:?}"
    );
    assert!(
        put_uris.iter().any(|u| u.contains("/slides/")),
        "PUT calls should include a URI with /slides/, got: {put_uris:?}"
    );

    // fetch_mapping targets an individual resource; fetch_mappings targets a query endpoint
    assert!(
        get_uris.iter().all(|u| u.contains("/empaia/") && !u.ends_with("/query")),
        "fetch_mapping URIs should contain '/empaia/' and not end with '/query', got: {get_uris:?}"
    );
    assert!(
        put_uris.iter().all(|u| u.ends_with("/query")),
        "fetch_mappings URIs should end with '/query', got: {put_uris:?}"
    );
}

/// Verifies REST call extraction from a complex real-world async file (`cycle.py`).
/// The file uses variable-assigned f-string URLs, deeply nested control flow, and
/// multiple HTTP methods (GET, PUT, POST). Symbolic evaluation must resolve the
/// local `url` variables correctly for each function.
#[test]
fn should_extract_rest_calls_from_complex_async_file() {
    let filename = "./examples/python/callgraph/cycle.py";
    let (code, tree) = parse_file(filename);
    let mut restcalls = restcalls(&code, &tree, filename);
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_local_and_global_assignments(&mut restcalls, &assignments_map);

    // Expected REST calls by function name prefix and HTTP method
    let cases: &[(&str, HttpMethod, &str)] = &[
        ("_fetch_jobs", HttpMethod::PUT, "/v1/jobs/query"),
        ("_update_job_status", HttpMethod::PUT, "/v1/jobs/"),
        ("_get_execution", HttpMethod::GET, "/v1/executions/"),
        ("_fetch_raw_token", HttpMethod::GET, "/v1/jobs/"),
        ("_create_execution_request_raw", HttpMethod::POST, "/v1/executions"),
    ];

    for (fn_prefix, expected_method, uri_fragment) in cases {
        let matching: Vec<&RestCall> = restcalls
            .iter()
            .filter(|r| r.function_name.starts_with(fn_prefix) && r.http_method == *expected_method)
            .collect();
        assert!(
            !matching.is_empty(),
            "expected a {:?} REST call in function '{fn_prefix}', got none. All calls: {:#?}",
            expected_method,
            restcalls.iter().map(|r| (&r.function_name, &r.http_method, &r.target_uri)).collect::<Vec<_>>()
        );
        assert!(
            matching.iter().any(|r| r.target_uri.contains(uri_fragment)),
            "expected URI containing '{uri_fragment}' in '{fn_prefix}', got: {:?}",
            matching.iter().map(|r| &r.target_uri).collect::<Vec<_>>()
        );
    }
}
