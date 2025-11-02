use python_extractor::{
    extraction::{
        endpoints::extractor::EndpointsExtractor,
        extractor::{ExtractParams, Extractor},
    },
    s,
    utils::load_file,
};

use crate::python::utils::get_tree;

use models::{Endpoint, HttpMethod, Parameter};

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
            http_method: HttpMethod::POST,
            parameters: vec![Parameter {
                name: s!("item"),
                datatype: Some(s!("ItemCreate")),
                initial_value: None,
            }],
            uri: s!("/items/"),
            file_path: s!(filename),
        },
        Endpoint {
            function_name: s!("read_items"),
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
        },
        Endpoint {
            function_name: s!("read_item"),
            http_method: HttpMethod::GET,
            parameters: vec![Parameter {
                name: s!("item_id"),
                datatype: Some(s!("int")),
                initial_value: Some(s!("0")),
            }],
            uri: s!("/items/{item_id}"),
            file_path: s!(filename),
        },
        Endpoint {
            function_name: s!("update_item"),
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
        },
        Endpoint {
            function_name: s!("delete_item"),
            http_method: HttpMethod::DELETE,
            parameters: vec![Parameter {
                name: s!("item_id"),
                datatype: Some(s!("int")),
                initial_value: None,
            }],
            uri: s!("/items/{item_id}"),
            file_path: s!(filename),
        },
        Endpoint {
            function_name: s!("create_user"),
            http_method: HttpMethod::POST,
            parameters: vec![Parameter {
                name: s!("user"),
                datatype: Some(s!("UserCreate")),
                initial_value: None,
            }],
            uri: s!("/users/"),
            file_path: s!(filename),
        },
        Endpoint {
            function_name: s!("list_users"),
            http_method: HttpMethod::GET,
            parameters: vec![Parameter {
                name: s!("limit"),
                datatype: Some(s!("int")),
                initial_value: Some(s!("Query(10, le=100)")),
            }],
            uri: s!("/users/"),
            file_path: s!(filename),
        },
        Endpoint {
            function_name: s!("get_user"),
            http_method: HttpMethod::GET,
            parameters: vec![Parameter {
                name: s!("user_id"),
                datatype: Some(s!("int")),
                initial_value: Some(s!("Path(..., gt=0)")),
            }],
            uri: s!("/users/{user_id}"),
            file_path: s!(filename),
        },
        Endpoint {
            function_name: s!("search"),
            http_method: HttpMethod::GET,
            parameters: vec![Parameter {
                name: s!("q"),
                datatype: Some(s!("str")),
                initial_value: Some(s!("Query(..., min_length=2)")),
            }],
            uri: s!("/search/"),
            file_path: s!(filename),
        },
    ];

    assert_eq!(endpoints, expected);
}
