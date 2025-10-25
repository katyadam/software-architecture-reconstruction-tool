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
    let filename = "./examples/python/restcalls/large_example.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let restcalls =
        RestcallsExtractor.extract(ExtractParams::new(&tree, &code).file_name(&filename));
    let expected = vec![
        RestCall {
            function_name: s!("create_item"),
            function_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("payload"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("{BASE_URL}/items/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_items"),
            function_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("params"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/items/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_item_by_id"),
            function_arguments: vec![],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/items/{item_id}"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("create_user"),
            function_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("payload"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("{BASE_URL}/users/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_users"),
            function_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"limit\": limit}"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/users/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("search"),
            function_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"q\": query}"),
            }],
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
    let mut restcalls =
        RestcallsExtractor.extract(ExtractParams::new(&tree, &code).file_name(&filename));
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
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_items"),
            function_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"skip\": skip, \"limit\": limit}"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_item_by_id"),
            function_arguments: vec![],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/items/{item_id}"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("create_user"),
            function_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("{\n        \"username\": username,\n        \"email\": email\n    }"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_users"),
            function_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"limit\": limit}"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("search"),
            function_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"q\": query}"),
            }],
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
    let mut restcalls =
        RestcallsExtractor.extract(ExtractParams::new(&tree, &code).file_name(&filename));
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_restcalls(&mut restcalls, assignments_map);
    let expected = vec![
        RestCall {
            function_name: s!("endpoint_with_withblock_restcall"),
            function_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("data.dict()"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("withblock_restcall_assignment"),
            function_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("data.dict()"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("restcall_assignment"),
            function_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"skip\": skip, \"limit\": limit}"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("restcall_no_assignment"),
            function_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("{\n        \"username\": username,\n        \"email\": email\n    }"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("await_restcall_assignment"),
            function_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("{\n        \"username\": username,\n        \"email\": email\n    }"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("await_restcall_no_assignment"),
            function_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("{\n        \"username\": username,\n        \"email\": email\n    }"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
    ];

    assert_eq!(restcalls, expected);
}
