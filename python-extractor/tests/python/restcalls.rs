use python_extractor::{
    extraction::{
        assignments::map::get_assignments_map,
        calls::extractor::CallsExtractor,
        extractor::{ExtractParams, Extractor},
        restcalls::{
            evaluation::method_call::MethodCallEvaluationStrategy,
            evaluator::evaluate_restcalls,
            identification::method_call::MethodCallIdentificationStrategy,
            selection::{method_call::MethodCallSelector, selector::Selector},
        },
    },
    s,
};

use models::{Argument, HttpMethod, RestCall};
use statix::{parse_python, python::matcher::PythonCallableMatcher};
use tree_sitter::Tree;

use crate::python::utils::{get_tree, load_file};

fn restcalls(code: &str, tree: &Tree, file_name: &str) -> Vec<RestCall> {
    let calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));
    let function_asts = parse_python(&tree, &code);
    MethodCallSelector::new(
        MethodCallIdentificationStrategy::new(),
        MethodCallEvaluationStrategy::new(function_asts),
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
    evaluate_restcalls(&mut restcalls, &assignments_map);
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
    evaluate_restcalls(&mut restcalls, &assignments_map);
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
    evaluate_restcalls(&mut restcalls, &assignments_map);

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
