use statix::{
    ast::Expr,
    java::matcher::JavaCallableMatcher,
    parse_java, symbolic_evaluation,
    util::{get_java_tree, load_file},
};

// ── Existing test ─────────────────────────────────────────────────────────────

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

// ── Parser: literal propagation ───────────────────────────────────────────────

#[test]
fn java_string_literal_declaration_and_return() {
    let code = r#"
class Svc {
    String getUrl() {
        String url = "http://example.com";
        return url;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String getUrl()",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://example.com".to_string()),
        "string literal should propagate through variable declaration to return site"
    );
}

#[test]
fn java_integer_literal_declaration_and_return() {
    let code = r#"
class Svc {
    int getPort() {
        int port = 8080;
        return port;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(&map, "int getPort()", Box::new(JavaCallableMatcher::new()))
        .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("8080".to_string()),
        "integer literal should propagate as its string representation"
    );
}

// ── Parser: string concatenation and constant folding ────────────────────────

#[test]
fn java_string_concatenation_folds_to_literal() {
    let code = r#"
class Svc {
    String buildUrl() {
        String base = "http://svc";
        String path = "/api/v1";
        String url = base + path;
        return url;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String buildUrl()",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://svc/api/v1".to_string()),
        "two String literals connected by + should be constant-folded into one Literal"
    );
}

#[test]
fn java_concat_with_param_produces_concat_expr() {
    let code = r#"
class Svc {
    String buildUrl(String host) {
        String path = "/orders";
        String url = host + path;
        return url;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String buildUrl(String)",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    if let Expr::Concat(lhs, rhs) = &result.return_value {
        assert_eq!(
            **lhs,
            Expr::Var("host".to_string()),
            "left side should be the unresolved param variable"
        );
        assert_eq!(
            **rhs,
            Expr::Literal("/orders".to_string()),
            "right side should be the resolved path literal"
        );
    } else {
        panic!("Expected Expr::Concat, got {:?}", result.return_value);
    }
}

#[test]
fn java_deeply_nested_concat_folds_to_single_literal() {
    let code = r#"
class Svc {
    String buildUrl() {
        String a = "http://";
        String b = "host";
        String c = "/path";
        String ab = a + b;
        String url = ab + c;
        return url;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String buildUrl()",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://host/path".to_string()),
        "multi-step constant folding: a+b then (a+b)+c should produce one Literal"
    );
}

// ── Evaluator: branch selection with boolean literals ────────────────────────

/// Java `true` becomes Expr::Literal("true"). The join function checks for the
/// lowercase "true" string, so Java true DOES trigger deterministic then-branch
/// selection. This contrasts with Python's `True` which does not.
#[test]
fn java_boolean_true_selects_then_branch() {
    let code = r#"
class Svc {
    String getEnv() {
        boolean isProd = true;
        String env = "";
        if (isProd) {
            env = "production";
        } else {
            env = "development";
        }
        return env;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String getEnv()",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("production".to_string()),
        "literal `true` condition should deterministically select the then-branch"
    );
}

#[test]
fn java_boolean_false_selects_else_branch() {
    let code = r#"
class Svc {
    String getEnv() {
        boolean isProd = false;
        String env = "";
        if (isProd) {
            env = "production";
        } else {
            env = "development";
        }
        return env;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String getEnv()",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("development".to_string()),
        "literal `false` condition should deterministically select the else-branch"
    );
}

#[test]
fn java_unknown_condition_joins_branches() {
    let code = r#"
class Svc {
    String getUrl(boolean useProd) {
        String url = "";
        if (useProd) {
            url = "http://prod";
        } else {
            url = "http://dev";
        }
        return url;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String getUrl(boolean)",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    if let Expr::Joined { vals } = &result.return_value {
        assert!(
            vals.contains(&Expr::Literal("http://prod".to_string())),
            "then-branch value should be in Joined"
        );
        assert!(
            vals.contains(&Expr::Literal("http://dev".to_string())),
            "else-branch value should be in Joined"
        );
        assert!(
            !vals.contains(&Expr::Literal(String::new())),
            "pre-if original value should NOT appear — both branches overwrite it"
        );
    } else {
        panic!(
            "Expected Expr::Joined for unknown boolean param condition, got {:?}",
            result.return_value
        );
    }
}

#[test]
fn java_if_with_no_else_and_false_condition_keeps_original_value() {
    let code = r#"
class Svc {
    String getUrl() {
        String url = "http://default";
        boolean flag = false;
        if (flag) {
            url = "http://override";
        }
        return url;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String getUrl()",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://default".to_string()),
        "false condition with no else: then-branch skipped, original value preserved"
    );
}

// ── Evaluator: cross-method call resolution ───────────────────────────────────

#[test]
fn java_zero_param_callee_is_resolved_cross_method() {
    let code = r#"
class Svc {
    String getBase() {
        return "http://base-service";
    }
    String getFullUrl() {
        String url = getBase();
        return url;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String getFullUrl()",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://base-service".to_string()),
        "zero-param callee should be resolved and its return literal inlined"
    );
}

#[test]
fn java_method_invocation_with_literal_arg_folds() {
    let code = r#"
class Svc {
    String getServiceUrl(String name) {
        return "http://" + name;
    }
    String buildUrl() {
        String serviceName = "my-service";
        String url = getServiceUrl(serviceName);
        return url;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String buildUrl()",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://my-service".to_string()),
        "literal argument passed to a callee should participate in constant folding"
    );
}

#[test]
fn java_unknown_method_invocation_produces_empty() {
    let code = r#"
class Svc {
    String getUrl() {
        String url = unknownMethod();
        return url;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String getUrl()",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("unknown method should not cause an error");

    assert_eq!(
        result.return_value,
        Expr::Empty,
        "call to an unknown method should produce Expr::Empty, not an error"
    );
}

// ── Parser: expression edge cases ────────────────────────────────────────────

#[test]
fn java_parenthesized_expression_is_transparent() {
    let code = r#"
class Svc {
    String getUrl() {
        String url = ("http://svc");
        return url;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String getUrl()",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://svc".to_string()),
        "parenthesized expression should be transparent — same as unparenthesized literal"
    );
}

#[test]
fn java_non_plus_binary_operator_produces_empty() {
    let code = r#"
class Svc {
    int compute() {
        int x = 10;
        int y = x * 2;
        return y;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(&map, "int compute()", Box::new(JavaCallableMatcher::new()))
        .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Empty,
        "non-+ binary operators are not supported and should yield Expr::Empty"
    );
}

// ── Parser: key format ────────────────────────────────────────────────────────

#[test]
fn java_parse_produces_correct_header_key() {
    let code = r#"
class Svc {
    String getUrl() {
        return "http://svc";
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);

    assert!(
        map.contains_key("String getUrl()"),
        "Java method header key should follow format 'ReturnType methodName(TypeList)', map keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
}

#[test]
fn java_parse_header_key_with_typed_params() {
    let code = r#"
class Svc {
    String buildUrl(String host, int port) {
        return host;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);

    assert!(
        map.contains_key("String buildUrl(String,int)"),
        "param types (without param names) should appear in mangled header key, map keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
}

// ── Evaluator: error and empty-body cases ─────────────────────────────────────

#[test]
fn java_empty_method_body_returns_empty() {
    let code = r#"
class Svc {
    String getUrl() {
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String getUrl()",
        Box::new(JavaCallableMatcher::new()),
    )
    .expect("empty body should not panic");

    assert_eq!(
        result.return_value,
        Expr::Empty,
        "method with no statements should yield Expr::Empty"
    );
}

#[test]
fn java_nonexistent_callable_returns_eval_error() {
    let code = r#"
class Svc {
    String getUrl() {
        return "http://svc";
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String doesNotExist()",
        Box::new(JavaCallableMatcher::new()),
    );

    assert!(
        result.is_err(),
        "requesting evaluation of a non-existent callable should return Err"
    );
}

#[test]
fn java_assignment_to_undeclared_variable_returns_eval_error() {
    let code = r#"
class Svc {
    String getUrl() {
        url = "http://svc";
        return url;
    }
}
"#;
    let tree = get_java_tree(code);
    let map = parse_java(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "String getUrl()",
        Box::new(JavaCallableMatcher::new()),
    );

    assert!(
        result.is_err(),
        "assigning to a variable that was never declared should return EvalError::NonSenseEvaluation"
    );
}
