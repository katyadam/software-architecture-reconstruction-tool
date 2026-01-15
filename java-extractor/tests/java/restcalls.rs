use java_extractor::{
    extraction::{
        assignments::map::get_assignments_map,
        calls::{evaluator::evaluate_invocations, extractor::CallStatementsExtractor},
        extractor::Extractor,
        restcalls::{
            identification::spring::SpringStrategy,
            selection::{selector::Selector, spring::SpringSelector},
        },
    },
    s,
};
use models::{Argument, HttpMethod, RestCall};

use crate::java::utils::{get_tree, load_file};

#[test]
fn test_spring_restcalls_without_dfa() {
    let filename = s!("./examples/CancelServiceImpl.java");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let mut calls = CallStatementsExtractor.extract(&code, &tree, &filename);
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_invocations(&mut calls, &assignments_map);

    let restcalls =
        SpringSelector::new(SpringStrategy::new()).select_restcall_statements(&calls, &filename);
    let expected = vec![
        RestCall {
            function_name: s!("sendEmail(NotifyInfo notifyInfo, HttpHeaders headers)"),
            function_hash: s!("8920b5600d64f9685efc83eb9367d881d398720c7a49e1e34d9b3cfbec57f41f"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!(
                        "notification_service_url + \"/api/v1/notifyservice/notification/order_cancel_success\""
                    ),
                    datatype: s!("String"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.POST"),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: s!("HttpEntity"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("Boolean.class"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::POST,
            target_uri: s!(
                "notification_service_url + \"/api/v1/notifyservice/notification/order_cancel_success\""
            ),
            file_path: s!("./examples/CancelServiceImpl.java"),
        },
        RestCall {
            function_name: s!("cancelFromOrder(Order order, HttpHeaders headers)"),
            function_hash: s!("3392f659271ac08950632de0cf65ab80aa11a850fc73a2221c82f1f0fe93e590"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("order_service_url + \"/api/v1/orderservice/order\""),
                    datatype: s!("String"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.PUT"),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: s!("HttpEntity"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("Response.class"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::PUT,
            target_uri: s!("order_service_url + \"/api/v1/orderservice/order\""),
            file_path: s!("./examples/CancelServiceImpl.java"),
        },
        RestCall {
            function_name: s!("cancelFromOtherOrder(Order order, HttpHeaders headers)"),
            function_hash: s!("480df4f76033d3019d79e96eeb06889f7b573049d611fdd4f78c816625f42443"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("order_other_service_url + \"/api/v1/orderOtherService/orderOther\""),
                    datatype: s!("String"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.PUT"),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: s!("HttpEntity"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("Response.class"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::PUT,
            target_uri: s!("order_other_service_url + \"/api/v1/orderOtherService/orderOther\""),
            file_path: s!("./examples/CancelServiceImpl.java"),
        },
        RestCall {
            function_name: s!("drawbackMoney(String money, String userId, HttpHeaders headers)"),
            function_hash: s!("8e60c557d0f1850afbe875e5dca5b914188914936c9b08da9b813adf9c84ac5f"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!(
                        "inside_payment_service_url + \"/api/v1/inside_pay_service/inside_payment/drawback/\" + userId + \"/\" + money"
                    ),
                    datatype: s!("String"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.GET"),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: s!("HttpEntity"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("Response.class"),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!(
                "inside_payment_service_url + \"/api/v1/inside_pay_service/inside_payment/drawback/\" + userId + \"/\" + money"
            ),
            file_path: s!("./examples/CancelServiceImpl.java"),
        },
        RestCall {
            function_name: s!("getAccount(String orderId, HttpHeaders headers)"),
            function_hash: s!("b203ff280f41c557f046cb438f62cefe99a0da9d250802448fe7aa6b5b303cb2"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("user_service_url + \"/api/v1/userservice/users/id/\" + orderId"),
                    datatype: s!("String"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.GET"),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: s!("HttpEntity"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!(
                        "new ParameterizedTypeReference<Response<User>>() {\n                }"
                    ),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!("user_service_url + \"/api/v1/userservice/users/id/\" + orderId"),
            file_path: s!("./examples/CancelServiceImpl.java"),
        },
        RestCall {
            function_name: s!("getOrderByIdFromOrder(String orderId, HttpHeaders headers)"),
            function_hash: s!("2fedaea877a99656bd0afb28f9523f5d645d84e4987d404199f5d0258615d3de"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("order_service_url + \"/api/v1/orderservice/order/\" + orderId"),
                    datatype: s!("String"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.GET"),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: s!("HttpEntity"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!(
                        "new ParameterizedTypeReference<Response<Order>>() {\n                }"
                    ),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!("order_service_url + \"/api/v1/orderservice/order/\" + orderId"),
            file_path: s!("./examples/CancelServiceImpl.java"),
        },
        RestCall {
            function_name: s!("getOrderByIdFromOrderOther(String orderId, HttpHeaders headers)"),
            function_hash: s!("c55aebff4c961d6985892061493c4456d81bc6a2dca255bb473774d14842092a"),
            call_arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!(
                        "order_other_service_url + \"/api/v1/orderOtherService/orderOther/\" + orderId"
                    ),
                    datatype: s!("String"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.GET"),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: s!("HttpEntity"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!(
                        "new ParameterizedTypeReference<Response<Order>>() {\n                }"
                    ),
                    datatype: s!("any"),
                },
            ],
            http_method: HttpMethod::GET,
            target_uri: s!(
                "order_other_service_url + \"/api/v1/orderOtherService/orderOther/\" + orderId"
            ),
            file_path: s!("./examples/CancelServiceImpl.java"),
        },
    ];

    assert_eq!(restcalls, expected);
}
