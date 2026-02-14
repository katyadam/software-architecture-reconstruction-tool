use statix::{
    ast::Expr,
    java::matcher::JavaCallableMatcher,
    parse_java, symbolic_evaluation,
    util::{get_java_tree, load_file},
};

#[test]
fn should_return_correct_variable_with_value_and_datatype() {
    let file_name = "./examples/RestCallAfterMethodInvocation.java".to_string();
    let code = load_file(&file_name).unwrap();
    let tree = get_java_tree(&code);
    let methods_map = parse_java(&tree, &code);

    let result = symbolic_evaluation(
        &methods_map,
        "boolean drawbackMoney(String,String,HttpHeaders)",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("This test should not fail!");
    assert_eq!(
        result.return_value,
        Expr::Empty,
        "Return expression should be empty, the method returns void"
    );
    let looked_variable = result
        .final_env
        .get("inside_payment_service_url")
        .expect("inside_payment_service_url should be in the final env!");
    assert_eq!(
        looked_variable.0, "String",
        "inside_payment_service_url should have datatype String"
    );
    assert_eq!(
        looked_variable.1,
        Expr::Literal("http://ts-inside-payment-service aa".to_string()),
        "inside_payment_service_url should have correct value"
    )
}
