use java_extractor::{
    extraction::{
        assignments::map::get_assignments_map,
        calls::{evaluator::evaluate_invocations, extractor::CallStatementsExtractor},
        extractor::Extractor,
        restcalls::{
            evaluation::spring::SpringEvaluationStrategy,
            identification::spring::SpringIdentificationStrategy,
            selection::{selector::Selector, spring::SpringSelector},
        },
    },
    s,
};
use models::{Argument, HttpMethod, RestCall, source_code::SourceSpan};
use statix::parse_java;

use crate::java::utils::{get_tree, load_file, parse_file};

#[test]
fn test_spring_restcalls_without_dfa() {
    let filename = s!("./examples/CancelServiceImpl.java");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let mut calls = CallStatementsExtractor.extract(&code, &tree, &filename);
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_invocations(&mut calls, &assignments_map);

    let method_asts = parse_java(&tree, &code);
    let restcalls = SpringSelector::new(
        SpringIdentificationStrategy::new(),
        SpringEvaluationStrategy::new(method_asts),
    )
    .select_restcall_statements(&calls, &filename)
    .expect("Evaluation should not fail!");

    let expected = vec![
        RestCall {
            function_name: s!("boolean sendEmail(NotifyInfo notifyInfo, HttpHeaders headers)"),
            function_hash: s!("8920b5600d64f9685efc83eb9367d881d398720c7a49e1e34d9b3cfbec57f41f"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!(
                        "notification_service_url + \"/api/v1/notifyservice/notification/order_cancel_success\""
                    ),
                    datatype: Some(s!("String")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.POST"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: Some(s!("HttpEntity")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("Boolean.class"),
                    datatype: None,
                },
            ],
            http_method: HttpMethod::POST,
            target_uri: s!(
                "http://ts-notification-service/api/v1/notifyservice/notification/order_cancel_success"
            ),
            file_path: s!("./examples/CancelServiceImpl.java"),
            source_span: SourceSpan { start_byte: 945, end_byte: 16898 },
        },
        RestCall {
            function_name: s!("Response cancelFromOrder(Order order, HttpHeaders headers)"),
            function_hash: s!("3392f659271ac08950632de0cf65ab80aa11a850fc73a2221c82f1f0fe93e590"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("order_service_url + \"/api/v1/orderservice/order\""),
                    datatype: Some(s!("String")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.PUT"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: Some(s!("HttpEntity")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("Response.class"),
                    datatype: None,
                },
            ],
            http_method: HttpMethod::PUT,
            target_uri: s!("http://ts-order-service/api/v1/orderservice/order"),
            file_path: s!("./examples/CancelServiceImpl.java"),
            source_span: SourceSpan { start_byte: 945, end_byte: 16898 },
        },
        RestCall {
            function_name: s!("Response cancelFromOtherOrder(Order order, HttpHeaders headers)"),
            function_hash: s!("480df4f76033d3019d79e96eeb06889f7b573049d611fdd4f78c816625f42443"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("order_other_service_url + \"/api/v1/orderOtherService/orderOther\""),
                    datatype: Some(s!("String")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.PUT"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: Some(s!("HttpEntity")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("Response.class"),
                    datatype: None,
                },
            ],
            http_method: HttpMethod::PUT,
            target_uri: s!("http://ts-order-other-service/api/v1/orderOtherService/orderOther"),
            file_path: s!("./examples/CancelServiceImpl.java"),
            source_span: SourceSpan { start_byte: 945, end_byte: 16898 },
        },
        RestCall {
            function_name: s!(
                "boolean drawbackMoney(String money, String userId, HttpHeaders headers)"
            ),
            function_hash: s!("8e60c557d0f1850afbe875e5dca5b914188914936c9b08da9b813adf9c84ac5f"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!(
                        "inside_payment_service_url + \"/api/v1/inside_pay_service/inside_payment/drawback/\" + userId + \"/\" + money"
                    ),
                    datatype: Some(s!("String")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.GET"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: Some(s!("HttpEntity")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("Response.class"),
                    datatype: None,
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!(
                "http://ts-inside-payment-service/api/v1/inside_pay_service/inside_payment/drawback/userId/money"
            ),
            file_path: s!("./examples/CancelServiceImpl.java"),
            source_span: SourceSpan { start_byte: 945, end_byte: 16898 },
        },
        RestCall {
            function_name: s!("Response<User> getAccount(String orderId, HttpHeaders headers)"),
            function_hash: s!("b203ff280f41c557f046cb438f62cefe99a0da9d250802448fe7aa6b5b303cb2"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("user_service_url + \"/api/v1/userservice/users/id/\" + orderId"),
                    datatype: Some(s!("String")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.GET"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: Some(s!("HttpEntity")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!(
                        "new ParameterizedTypeReference<Response<User>>() {\n                }"
                    ),
                    datatype: None,
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!("http://ts-user-service/api/v1/userservice/users/id/orderId"),
            file_path: s!("./examples/CancelServiceImpl.java"),
            source_span: SourceSpan { start_byte: 945, end_byte: 16898 },
        },
        RestCall {
            function_name: s!(
                "Response<Order> getOrderByIdFromOrder(String orderId, HttpHeaders headers)"
            ),
            function_hash: s!("2fedaea877a99656bd0afb28f9523f5d645d84e4987d404199f5d0258615d3de"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("order_service_url + \"/api/v1/orderservice/order/\" + orderId"),
                    datatype: Some(s!("String")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.GET"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: Some(s!("HttpEntity")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!(
                        "new ParameterizedTypeReference<Response<Order>>() {\n                }"
                    ),
                    datatype: None,
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!("http://ts-order-service/api/v1/orderservice/order/orderId"),
            file_path: s!("./examples/CancelServiceImpl.java"),
            source_span: SourceSpan { start_byte: 945, end_byte: 16898 },
        },
        RestCall {
            function_name: s!(
                "Response<Order> getOrderByIdFromOrderOther(String orderId, HttpHeaders headers)"
            ),
            function_hash: s!("c55aebff4c961d6985892061493c4456d81bc6a2dca255bb473774d14842092a"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!(
                        "order_other_service_url + \"/api/v1/orderOtherService/orderOther/\" + orderId"
                    ),
                    datatype: Some(s!("String")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.GET"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: Some(s!("HttpEntity")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!(
                        "new ParameterizedTypeReference<Response<Order>>() {\n                }"
                    ),
                    datatype: None,
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!(
                "http://ts-order-other-service/api/v1/orderOtherService/orderOther/orderId"
            ),
            file_path: s!("./examples/CancelServiceImpl.java"),
            source_span: SourceSpan { start_byte: 945, end_byte: 16898 },
        },
    ];

    assert_eq!(restcalls, expected);
}

#[test]
fn test_double_pointing_spring_restcalls() {
    let filename = s!("./examples/DoublePointingRestCalls.java");
    let (code, tree) = parse_file(&filename);
    let mut calls = CallStatementsExtractor.extract(&code, &tree, &filename);
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_invocations(&mut calls, &assignments_map);

    let method_asts = parse_java(&tree, &code);
    let restcalls = SpringSelector::new(
        SpringIdentificationStrategy::new(),
        SpringEvaluationStrategy::new(method_asts),
    )
    .select_restcall_statements(&calls, &filename)
    .expect("Evaluation should not fail!");

    assert_eq!(
        restcalls.len(),
        8,
        "Expected 8 REST calls (4 methods × 2 branches each)"
    );

    // Verify all 4 HTTP methods are represented (DELETE is key new coverage)
    let methods: Vec<&HttpMethod> = restcalls.iter().map(|rc| &rc.http_method).collect();
    assert_eq!(
        methods.iter().filter(|&&m| m == &HttpMethod::GET).count(),
        2,
        "Expected 2 GET calls"
    );
    assert_eq!(
        methods
            .iter()
            .filter(|&&m| m == &HttpMethod::DELETE)
            .count(),
        2,
        "Expected 2 DELETE calls"
    );
    assert_eq!(
        methods.iter().filter(|&&m| m == &HttpMethod::PUT).count(),
        2,
        "Expected 2 PUT calls"
    );
    assert_eq!(
        methods.iter().filter(|&&m| m == &HttpMethod::POST).count(),
        2,
        "Expected 2 POST calls"
    );

    // getAllOrders: 2× GET (ts-order-service and ts-order-other-service)
    let get_all_orders: Vec<&RestCall> = restcalls
        .iter()
        .filter(|rc| rc.function_name.contains("getAllOrders"))
        .collect();
    assert_eq!(get_all_orders.len(), 2);
    assert!(
        get_all_orders
            .iter()
            .any(|rc| rc.target_uri.contains("ts-order-service")
                && rc.target_uri.contains("/orderservice/order"))
    );
    assert!(
        get_all_orders
            .iter()
            .any(|rc| rc.target_uri.contains("ts-order-other-service")
                && rc.target_uri.contains("/orderOtherService/orderOther"))
    );

    // deleteOrder: 2× DELETE (ts-order-service and ts-order-other-service)
    let delete_order: Vec<&RestCall> = restcalls
        .iter()
        .filter(|rc| rc.function_name.contains("deleteOrder"))
        .collect();
    assert_eq!(delete_order.len(), 2);
    assert!(
        delete_order
            .iter()
            .all(|rc| rc.http_method == HttpMethod::DELETE)
    );
    assert!(
        delete_order
            .iter()
            .any(|rc| rc.target_uri.contains("ts-order-service"))
    );
    assert!(
        delete_order
            .iter()
            .any(|rc| rc.target_uri.contains("ts-order-other-service"))
    );

    // updateOrder: 2× PUT
    let update_order: Vec<&RestCall> = restcalls
        .iter()
        .filter(|rc| rc.function_name.contains("updateOrder"))
        .collect();
    assert_eq!(update_order.len(), 2);
    assert!(
        update_order
            .iter()
            .all(|rc| rc.http_method == HttpMethod::PUT)
    );
    assert!(
        update_order
            .iter()
            .any(|rc| rc.target_uri.contains("/orderservice/order/admin"))
    );
    assert!(update_order.iter().any(|rc| {
        rc.target_uri
            .contains("/orderOtherService/orderOther/admin")
    }));

    // addOrder: 2× POST
    let add_order: Vec<&RestCall> = restcalls
        .iter()
        .filter(|rc| rc.function_name.contains("addOrder"))
        .collect();
    assert_eq!(add_order.len(), 2);
    assert!(
        add_order
            .iter()
            .all(|rc| rc.http_method == HttpMethod::POST)
    );
    assert!(
        add_order
            .iter()
            .any(|rc| rc.target_uri.contains("/orderservice/order/admin"))
    );
    assert!(add_order.iter().any(|rc| {
        rc.target_uri
            .contains("/orderOtherService/orderOther/admin")
    }));
}

#[test]
fn test_restcall_edge_cases() {
    let filename = s!("./examples/RestCallEdgeCases.java");
    let (code, tree) = parse_file(&filename);
    let mut calls = CallStatementsExtractor.extract(&code, &tree, &filename);
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_invocations(&mut calls, &assignments_map);

    let method_asts = parse_java(&tree, &code);
    let restcalls = SpringSelector::new(
        SpringIdentificationStrategy::new(),
        SpringEvaluationStrategy::new(method_asts),
    )
    .select_restcall_statements(&calls, &filename)
    .expect("Evaluation should not fail!");

    assert_eq!(
        restcalls.len(),
        4,
        "Expected 4 REST calls: 1 PATCH + 2 GET (syncUserAndOrder) + 1 GET (getOrderItem)"
    );

    // Edge case 1: PATCH call — requires fixed FromStr arm for PATCH
    let patch_calls: Vec<&RestCall> = restcalls
        .iter()
        .filter(|rc| rc.http_method == HttpMethod::PATCH)
        .collect();
    assert_eq!(patch_calls.len(), 1, "Expected exactly 1 PATCH call");
    assert!(patch_calls[0].function_name.contains("updateUserPartial"));
    assert!(patch_calls[0].target_uri.contains("ts-user-service"));
    assert!(patch_calls[0].target_uri.contains("/api/v1/users/"));

    // Edge case 2: Two calls extracted from the same method
    let sync_calls: Vec<&RestCall> = restcalls
        .iter()
        .filter(|rc| rc.function_name.contains("syncUserAndOrder"))
        .collect();
    assert_eq!(
        sync_calls.len(),
        2,
        "Both REST calls inside syncUserAndOrder must be extracted"
    );
    assert!(
        sync_calls
            .iter()
            .any(|rc| rc.target_uri.contains("ts-user-service"))
    );
    assert!(
        sync_calls
            .iter()
            .any(|rc| rc.target_uri.contains("ts-order-service"))
    );

    // Edge case 3: URL with multiple concatenated path parameters
    let item_calls: Vec<&RestCall> = restcalls
        .iter()
        .filter(|rc| rc.function_name.contains("getOrderItem"))
        .collect();
    assert_eq!(item_calls.len(), 1);
    // Both orderId and itemId must appear in the resolved URI
    assert!(
        item_calls[0].target_uri.contains("orderId"),
        "orderId path param should appear in URI"
    );
    assert!(
        item_calls[0].target_uri.contains("itemId"),
        "itemId path param should appear in URI"
    );
    assert!(
        item_calls[0].target_uri.contains("/orders/"),
        "URI should contain /orders/ segment"
    );
    assert!(
        item_calls[0].target_uri.contains("/items/"),
        "URI should contain /items/ segment"
    );
}
