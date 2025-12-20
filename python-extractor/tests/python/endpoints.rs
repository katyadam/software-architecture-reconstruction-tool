use python_extractor::{
    extraction::{
        endpoints::extractor::EndpointsExtractor,
        extractor::{ExtractParams, Extractor},
    },
    s,
};

use crate::python::utils::{get_tree, load_file};

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
            function_hash: s!("912172a93ead73ec76396b25cca3afc21232de1fd4bf9611f0cbedca05089c17"),

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
        },
    ];

    assert_eq!(endpoints, expected);
}
