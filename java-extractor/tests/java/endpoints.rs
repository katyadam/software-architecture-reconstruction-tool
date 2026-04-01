use java_extractor::{
    extraction::{endpoints::extractor::EndpointsExtractor, extractor::Extractor},
    s,
};
use models::{Endpoint, HttpMethod, Parameter};

use crate::java::utils::{get_tree, load_file, parse_file};

#[test]
fn test_spring_endpoints() {
    let filename = s!("./examples/FoodController.java");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let endpoints = EndpointsExtractor.extract(&code, &tree, &filename);

    let expected = vec![
        Endpoint {
            function_name: s!("String home()"),
            function_hash: s!("045a36e9dca12d9a9140d8f6bb4341ab7e328984662c9019b72e00cf7504ca57"),
            http_method: HttpMethod::GET,
            parameters: vec![],
            uri: s!("/api/v1/foodservice/welcome"),
            file_path: s!("./examples/FoodController.java"),
            router_variable: None,
        },
        Endpoint {
            function_name: s!("boolean test_send_delivery()"),
            function_hash: s!("3f5d2ea2debdf3fe3398710182abae56fa4e0dd51e7fb2e3273a1b94a866c478"),
            http_method: HttpMethod::GET,
            parameters: vec![],
            uri: s!("/api/v1/foodservice/test_send_delivery"),
            file_path: s!("./examples/FoodController.java"),
            router_variable: None,
        },
        Endpoint {
            function_name: s!("HttpEntity findAllFoodOrder(@RequestHeader HttpHeaders headers)"),
            function_hash: s!("37b85b0b40671ce7c9b1bc6d6c195686a7ec9e6f5862a229451a319942b094d2"),
            http_method: HttpMethod::GET,
            parameters: vec![Parameter {
                name: s!("headers"),
                datatype: Some(s!("HttpHeaders")),
                initial_value: None,
            }],
            uri: s!("/api/v1/foodservice/orders"),
            file_path: s!("./examples/FoodController.java"),
            router_variable: None,
        },
        Endpoint {
            function_name: s!(
                "HttpEntity createFoodOrder(@RequestBody FoodOrder addFoodOrder, @RequestHeader HttpHeaders headers)"
            ),
            function_hash: s!("c2f76eee33b36afbf186ba7a934a14ba4a429f1d02ad42073d2d9efa71f91e68"),
            http_method: HttpMethod::POST,
            parameters: vec![
                Parameter {
                    name: s!("addFoodOrder"),
                    datatype: Some(s!("FoodOrder")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("headers"),
                    datatype: Some(s!("HttpHeaders")),
                    initial_value: None,
                },
            ],
            uri: s!("/api/v1/foodservice/orders"),
            file_path: s!("./examples/FoodController.java"),
            router_variable: None,
        },
        Endpoint {
            function_name: s!(
                "HttpEntity createFoodBatches(@RequestBody List<FoodOrder> foodOrderList, @RequestHeader HttpHeaders headers)"
            ),
            function_hash: s!("0c4ab9d73b8b0ff4830e8f1df8107104e1f1e49f0dcd741479d62555b2f8d415"),
            http_method: HttpMethod::POST,
            parameters: vec![
                Parameter {
                    name: s!("foodOrderList"),
                    datatype: Some(s!("List<FoodOrder>")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("headers"),
                    datatype: Some(s!("HttpHeaders")),
                    initial_value: None,
                },
            ],
            uri: s!("/api/v1/foodservice/createOrderBatch"),
            file_path: s!("./examples/FoodController.java"),
            router_variable: None,
        },
        Endpoint {
            function_name: s!(
                "HttpEntity updateFoodOrder(@RequestBody FoodOrder updateFoodOrder, @RequestHeader HttpHeaders headers)"
            ),
            function_hash: s!("5a3f1afec87ce827aac060c17c5c68887dd9d04ad7c5626b6eaaae47dab81383"),
            http_method: HttpMethod::PUT,
            parameters: vec![
                Parameter {
                    name: s!("updateFoodOrder"),
                    datatype: Some(s!("FoodOrder")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("headers"),
                    datatype: Some(s!("HttpHeaders")),
                    initial_value: None,
                },
            ],
            uri: s!("/api/v1/foodservice/orders"),
            file_path: s!("./examples/FoodController.java"),
            router_variable: None,
        },
        Endpoint {
            function_name: s!(
                "HttpEntity deleteFoodOrder(@PathVariable String orderId, @RequestHeader HttpHeaders headers)"
            ),
            function_hash: s!("42215edeb89d88da729f220d7143af3b5298dce7dbc15a6e6121c080c14271de"),
            http_method: HttpMethod::DELETE,
            parameters: vec![
                Parameter {
                    name: s!("orderId"),
                    datatype: Some(s!("String")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("headers"),
                    datatype: Some(s!("HttpHeaders")),
                    initial_value: None,
                },
            ],
            uri: s!("/api/v1/foodservice/orders/{orderId}"),
            file_path: s!("./examples/FoodController.java"),
            router_variable: None,
        },
        Endpoint {
            function_name: s!(
                "HttpEntity findFoodOrderByOrderId(@PathVariable String orderId, @RequestHeader HttpHeaders headers)"
            ),
            function_hash: s!("ff99646894cc79daee2ea7fc03ef34920f56155f7a872ac0f8d052452b6b3c9c"),
            http_method: HttpMethod::GET,
            parameters: vec![
                Parameter {
                    name: s!("orderId"),
                    datatype: Some(s!("String")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("headers"),
                    datatype: Some(s!("HttpHeaders")),
                    initial_value: None,
                },
            ],
            uri: s!("/api/v1/foodservice/orders/{orderId}"),
            file_path: s!("./examples/FoodController.java"),
            router_variable: None,
        },
        Endpoint {
            function_name: s!(
                "HttpEntity getAllFood(@PathVariable String date, @PathVariable String startStation, @PathVariable String endStation, @PathVariable String tripId, @RequestHeader HttpHeaders headers)"
            ),
            function_hash: s!("a471b703262df7f3df0d2f882d32d0bf89378415d851b6de9eef0dde69f210e2"),
            http_method: HttpMethod::GET,
            parameters: vec![
                Parameter {
                    name: s!("date"),
                    datatype: Some(s!("String")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("startStation"),
                    datatype: Some(s!("String")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("endStation"),
                    datatype: Some(s!("String")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("tripId"),
                    datatype: Some(s!("String")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("headers"),
                    datatype: Some(s!("HttpHeaders")),
                    initial_value: None,
                },
            ],
            uri: s!("/api/v1/foodservice/foods/{date}/{startStation}/{endStation}/{tripId}"),
            file_path: s!("./examples/FoodController.java"),
            router_variable: None,
        },
    ];

    assert_eq!(endpoints, expected);
}

#[test]
fn test_jaxrs_endpoints() {
    let filename = s!("./examples/CallGraphController.java");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let endpoints = EndpointsExtractor.extract(&code, &tree, &filename);

    let expected = vec![
        Endpoint {
            function_name: s!(
                "Response getProjectAnalysisInputs(@PathParam(\"projectId\") Long projectId)"
            ),
            function_hash: s!("63e5ad0851266e62cdafe397a77fdb38ae51b889c041a6e1b23cd2b7387386af"),
            http_method: HttpMethod::GET,
            parameters: vec![Parameter {
                name: s!("projectId"),
                datatype: Some(s!("Long")),
                initial_value: None,
            }],
            uri: s!("/call-graph-inputs/project/{projectId}"),
            file_path: s!("./examples/CallGraphController.java"),
            router_variable: None,
        },
        Endpoint {
            function_name: s!(
                "Response getCallGraphInputById(@PathParam(\"callGraphInputId\") Long callGraphInputId)"
            ),
            function_hash: s!("fba7bb58a2b6a72ec9500307fe8471ec96c3ff65ca04a33861db20086b97c16d"),
            http_method: HttpMethod::GET,
            parameters: vec![Parameter {
                name: s!("callGraphInputId"),
                datatype: Some(s!("Long")),
                initial_value: None,
            }],
            uri: s!("/call-graph-inputs/{callGraphInputId}"),
            file_path: s!("./examples/CallGraphController.java"),
            router_variable: None,
        },
        Endpoint {
            function_name: s!(
                "Response getSummary(@PathParam(\"callGraphInputId\") Long callGraphInputId)"
            ),
            function_hash: s!("b7980091b00be8fc5cee4260c8c42bb383ef27cb7b46b3a4694549804d41c670"),
            http_method: HttpMethod::GET,
            parameters: vec![Parameter {
                name: s!("callGraphInputId"),
                datatype: Some(s!("Long")),
                initial_value: None,
            }],
            uri: s!("/call-graph-inputs/{callGraphInputId}/summary"),
            file_path: s!("./examples/CallGraphController.java"),
            router_variable: None,
        },
        Endpoint {
            function_name: s!(
                "Uni<CallGraph> computeMethodReachability( @PathParam(\"callGraphInputId\") Long callGraphInputId, @Valid CallGraphMethodKey callGraphMethodKey )"
            ),
            function_hash: s!("79c9955e49252b5ee218775c4cf0a453eba09d5f07dd447b32f368b9d220aa31"),
            http_method: HttpMethod::PUT,
            parameters: vec![
                Parameter {
                    name: s!("callGraphInputId"),
                    datatype: Some(s!("Long")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("callGraphMethodKey"),
                    datatype: Some(s!("CallGraphMethodKey")),
                    initial_value: None,
                },
            ],
            uri: s!("/call-graph-inputs/{callGraphInputId}/method-reachability"),
            file_path: s!("./examples/CallGraphController.java"),
            router_variable: None,
        },
        Endpoint {
            function_name: s!(
                "Response addCallGraphInput(@Valid CallGraphInputDto callGraphInputDto)"
            ),
            function_hash: s!("0219e6f3c235a25ee313cf2e523fa11da61248dfd8d2c74784704bcf1b58f98b"),
            http_method: HttpMethod::POST,
            parameters: vec![Parameter {
                name: s!("callGraphInputDto"),
                datatype: Some(s!("CallGraphInputDto")),
                initial_value: None,
            }],
            uri: s!("/call-graph-inputs"),
            file_path: s!("./examples/CallGraphController.java"),
            router_variable: None,
        },
        Endpoint {
            function_name: s!("Response deleteCallGraphInputById(@PathParam(\"id\") Long id)"),
            function_hash: s!("86cda4106cc818f43126e22214d02ff7180584125ff178b7b69d5440d6d5a31a"),
            http_method: HttpMethod::DELETE,
            parameters: vec![Parameter {
                name: s!("id"),
                datatype: Some(s!("Long")),
                initial_value: None,
            }],
            uri: s!("/call-graph-inputs/{id}"),
            file_path: s!("./examples/CallGraphController.java"),
            router_variable: None,
        },
    ];

    assert_eq!(endpoints, expected);
}

#[test]
fn test_endpoint_edge_cases() {
    let filename = s!("./examples/EndpointEdgeCases.java");
    let (code, tree) = parse_file(&filename);
    let endpoints = EndpointsExtractor.extract(&code, &tree, &filename);

    assert_eq!(
        endpoints.len(),
        3,
        "Expected 3 endpoints: PATCH, GET (nested path), GET (default from @RequestMapping)"
    );

    // Edge case 1: @PatchMapping produces a PATCH endpoint
    let patch_endpoints: Vec<&Endpoint> = endpoints
        .iter()
        .filter(|e| e.http_method == HttpMethod::PATCH)
        .collect();
    assert_eq!(
        patch_endpoints.len(),
        1,
        "Expected exactly 1 PATCH endpoint"
    );
    assert!(patch_endpoints[0].function_name.contains("partialUpdate"));
    assert_eq!(patch_endpoints[0].uri, s!("/api/v1/products/{id}"));

    // Edge case 2: Multi-level nested path with two path variables
    let review_endpoint = endpoints
        .iter()
        .find(|e| e.function_name.contains("getProductReview"))
        .unwrap();
    assert_eq!(review_endpoint.http_method, HttpMethod::GET);
    assert_eq!(
        review_endpoint.uri,
        s!("/api/v1/products/{productId}/reviews/{reviewId}")
    );

    // Edge case 3: @RequestMapping without method attribute defaults to GET
    // Documents that bare @RequestMapping carries no HTTP method information.
    let search_endpoint = endpoints
        .iter()
        .find(|e| e.function_name.contains("search"))
        .unwrap();
    assert_eq!(
        search_endpoint.http_method,
        HttpMethod::GET,
        "@RequestMapping without method= defaults to GET"
    );
    assert_eq!(search_endpoint.uri, s!("/api/v1/products/search"));
}
