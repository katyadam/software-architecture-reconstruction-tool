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
            function_name: s!("create_item(client, name, description, price, in_stock=True)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("payload"),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("{BASE_URL}/items/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_items(client, skip=0, limit=10, q=None)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("params"),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/items/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_item_by_id(client, item_id)"),
            call_arguments: vec![],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/items/{item_id}"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("create_user(client, username, email)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("payload"),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("{BASE_URL}/users/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_users(client, limit=10)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"limit\": limit}"),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("{BASE_URL}/users/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("search(client, query)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"q\": query}"),
                datatype: s!("any"),
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
    evaluate_restcalls(&mut restcalls, &assignments_map);
    let expected = vec![
        RestCall {
            function_name: s!("create_item(client, name, description, price, in_stock=True)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!(
                    "{\n        \"name\": name,\n        \"description\": description,\n        \"price\": price,\n        \"in_stock\": in_stock\n    }"
                ),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_items(client, skip=0, limit=10, q=None)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"skip\": skip, \"limit\": limit}"),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_item_by_id(client, item_id)"),
            call_arguments: vec![],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/items/{item_id}"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("create_user(client, username, email)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("{\n        \"username\": username,\n        \"email\": email\n    }"),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("get_users(client, limit=10)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"limit\": limit}"),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!(filename),
        },
        RestCall {
            function_name: s!("search(client, query)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"q\": query}"),
                datatype: s!("any"),
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
    evaluate_restcalls(&mut restcalls, &assignments_map);
    let expected = vec![
        RestCall {
            function_name: s!("endpoint_with_withblock_restcall(data: ProxyItemCreate)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("data.dict()"),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("withblock_restcall_assignment(data: ProxyItemCreate)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("data.dict()"),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("restcall_assignment(client, skip=0, limit=10, q=None)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("params"),
                value: s!("{\"skip\": skip, \"limit\": limit}"),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::GET,
            target_uri: s!("http://localhost:8000/items/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("restcall_no_assignment(client, username, email)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("{\n        \"username\": username,\n        \"email\": email\n    }"),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("await_restcall_assignment(client, username, email)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("{\n        \"username\": username,\n        \"email\": email\n    }"),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
        RestCall {
            function_name: s!("await_restcall_no_assignment(client, username, email)"),
            call_arguments: vec![Argument {
                assigned_variable: s!("json"),
                value: s!("{\n        \"username\": username,\n        \"email\": email\n    }"),
                datatype: s!("any"),
            }],
            http_method: HttpMethod::POST,
            target_uri: s!("http://localhost:8000/users/"),
            file_path: s!("./examples/python/restcalls/different_types.py"),
        },
    ];

    assert_eq!(restcalls, expected);
}
