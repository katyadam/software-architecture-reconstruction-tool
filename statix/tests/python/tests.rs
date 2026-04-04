use models::ir::ast::Expr;
use statix::{
    parse_python,
    python::matcher::PythonCallableMatcher,
    symbolic_evaluation,
    util::{get_python_tree, load_file},
};

// ── Existing tests ────────────────────────────────────────────────────────────

#[test]
fn parsing_should_not_fail_for_python_assignment_without_rhs() {
    let file_name = "./examples/uni_assignment_while_parsing.py".to_string();
    let code = load_file(&file_name).unwrap();
    let tree = get_python_tree(&code);
    let methods_map = parse_python(&tree, &code);

    assert!(!methods_map.is_empty());
}

#[test]
fn should_eval_statements_inside_try_except_block() {
    let file_name = "./examples/enum_in_restcall_uri.py".to_string();
    let code = load_file(&file_name).unwrap();
    let tree = get_python_tree(&code);
    let methods_map = parse_python(&tree, &code);

    let result = symbolic_evaluation(
        &methods_map,
        "Tuple[str, str] fetch_mapping(str,MappingType)",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("This test should not fail!");

    result
        .final_env
        .get("url")
        .expect("url should be in the final env!");
}

// ── Parser: literal propagation ───────────────────────────────────────────────

#[test]
fn python_string_literal_declaration_and_return() {
    let code = r#"
def get_url() -> str:
    url = "http://example.com"
    return url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str get_url()",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://example.com".to_string()),
        "string literal should propagate to return site"
    );
}

#[test]
fn python_integer_literal_declaration_and_return() {
    let code = r#"
def get_port() -> int:
    port = 8080
    return port
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "int get_port()",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("8080".to_string()),
        "integer literal should propagate as string representation"
    );
}

// ── Parser: string concatenation and folding ──────────────────────────────────

/// Direct literal concatenation in a return statement DOES constant-fold because
/// visit_literal returns type "String". Concatenation through intermediate variables
/// does NOT fold because Python untyped variables are stored with dtype "Any",
/// and the fold check requires concat_datatype == "String".
#[test]
fn python_direct_literal_concatenation_folds_to_literal() {
    let code = r#"
def build_url(self) -> str:
    return "http://svc" + "/api/v1"
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str build_url(Any)",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://svc/api/v1".to_string()),
        "two string literals directly joined with + should be constant-folded into one Literal"
    );
}

/// Concatenation via intermediate untyped Python variables does NOT fold because
/// Python variables without type annotations are stored with dtype "Any" in env,
/// and the constant-folding condition requires concat_datatype == "String".
#[test]
fn python_concat_via_untyped_variables_does_not_fold() {
    let code = r#"
def build_url(self) -> str:
    base = "http://svc"
    path = "/api/v1"
    url = base + path
    return url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str build_url(Any)",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert!(
        matches!(result.return_value, Expr::Concat(..)),
        "untyped variables use dtype 'Any' so the String-only fold check does not apply — expected Concat, got {:?}",
        result.return_value
    );
}

#[test]
fn python_concat_with_param_produces_concat_expr() {
    let code = r#"
def build_url(self, host: str) -> str:
    path = "/api"
    url = host + path
    return url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str build_url(Any,str)",
        Box::new(PythonCallableMatcher::new()),
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
            Expr::Literal("/api".to_string()),
            "right side should be the resolved literal"
        );
    } else {
        panic!("Expected Expr::Concat, got {:?}", result.return_value);
    }
}

// ── Parser: if-branch joining ─────────────────────────────────────────────────

#[test]
fn python_true_literal_condition_selects_then_branch() {
    let code = r#"
def get_flag(self) -> str:
    flag = True
    result = "initial"
    if flag:
        result = "yes"
    else:
        result = "no"
    return result
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str get_flag(Any)",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("yes".to_string()),
        "Python `True` should deterministically select the then-branch"
    );
}

/// When the condition is an unresolved parameter (Var), the evaluator joins the
/// then-branch value with the pre-if value (else branch is not parsed — see
/// parse_if limitation: else_clause node is passed instead of its body).
#[test]
fn python_unknown_condition_joins_then_and_original_value() {
    let code = r#"
def get_url(self, use_prod: bool) -> str:
    url = "http://default"
    if use_prod:
        url = "http://prod"
    return url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str get_url(Any,bool)",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    if let Expr::Joined { vals } = &result.return_value {
        assert!(
            vals.contains(&Expr::Literal("http://prod".to_string())),
            "Joined should contain the then-branch value"
        );
        assert!(
            vals.contains(&Expr::Literal("http://default".to_string())),
            "Joined should contain the original pre-if value (else side)"
        );
    } else {
        panic!(
            "Expected Expr::Joined for unknown condition, got {:?}",
            result.return_value
        );
    }
}

#[test]
fn python_unknown_condition_joins_then_and_else_branch_values() {
    let code = r#"
def get_url(self, use_prod: bool) -> str:
    url = "http://default"
    if use_prod:
        url = "http://prod"
    else:
        url = "http://dev"
    return url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str get_url(Any,bool)",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    // With unknown condition: join(Var("use_prod"), "http://prod", "http://dev")
    // -> Joined{"http://prod", "http://dev"}.
    // "http://default" does NOT appear — both branches overwrite it.
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
            !vals.contains(&Expr::Literal("http://default".to_string())),
            "pre-if original value should NOT appear — both branches overwrite it"
        );
    } else {
        panic!("Expected Expr::Joined, got {:?}", result.return_value);
    }
}

#[test]
fn python_if_without_else_true_condition_selects_then_branch() {
    let code = r#"
def maybe_override(self) -> str:
    url = "http://default"
    if True:
        url = "http://override"
    return url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str maybe_override(Any)",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://override".to_string()),
        "True condition with no else: then-branch wins deterministically"
    );
}

// ── Evaluator: cross-function call resolution ─────────────────────────────────

#[test]
fn python_zero_param_callee_is_resolved_cross_function() {
    let code = r#"
def get_base() -> str:
    return "http://base-service"

def get_full_url() -> str:
    url = get_base()
    return url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str get_full_url()",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://base-service".to_string()),
        "zero-param callee should be resolved and its return literal inlined"
    );
}

#[test]
fn python_function_invocation_with_literal_args_fold() {
    let code = r#"
def get_base(service_name: str) -> str:
    return "http://" + service_name

def get_full_url() -> str:
    service_name: str = "order-service"
    url = get_base(service_name)
    return url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str get_full_url()",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://order-service".to_string()),
        "zero-param callee should be resolved and its return literal inlined"
    );
}

/// Cross-function call resolution works when the callee's param type matches
/// the arg type in the call site. `visit_literal` returns type "String", but
/// Python params use "str", so literal args cannot match Python params. Passing
/// a `str`-typed variable arg DOES match, allowing the matcher to find the callee.
#[test]
fn python_typed_param_callee_is_resolved_cross_function() {
    let code = r#"
def make_url(host: str) -> str:
    return "http://" + host

def get_full_url(service: str) -> str:
    url = make_url(service)
    return url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str get_full_url(str)",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    // `service` has type "str" matching make_url's param type "str".
    // The matcher resolves make_url and inlines its body. The callee returns
    // Concat(Literal("http://"), Var("service")) — partially folded since
    // the arg is an unresolved variable, not a literal.
    if let Expr::Concat(lhs, _) = &result.return_value {
        assert_eq!(
            **lhs,
            Expr::Literal("http://".to_string()),
            "the callee's literal prefix should be preserved after inlining"
        );
    } else {
        panic!(
            "Expected Expr::Concat from the resolved callee's body, got {:?}",
            result.return_value
        );
    }
}

#[test]
fn python_unknown_function_call_produces_empty() {
    let code = r#"
def get_url(self) -> str:
    url = unknown_function()
    return url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str get_url(Any)",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("unknown function should not cause an error — it returns Empty");

    assert_eq!(
        result.return_value,
        Expr::Empty,
        "call to unknown function should produce Expr::Empty, not an error"
    );
}

// ── Parser: unsupported operators ─────────────────────────────────────────────

#[test]
fn python_non_plus_operator_produces_empty() {
    let code = r#"
def compute(self) -> int:
    x = 10
    y = x * 2
    return y
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "int compute(Any)",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Empty,
        "non-+ binary operators are not supported and should produce Expr::Empty"
    );
}

// ── Evaluator: try/except ─────────────────────────────────────────────────────

#[test]
fn python_try_except_joins_both_branches() {
    let code = r#"
def get_url() -> str:
    try:
        url = "http://service"
    except Exception:
        url = "http://fallback"
    return url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str get_url()",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    // try/except is modelled as if with unknown condition, so both branches are joined
    if let Expr::Joined { vals } = &result.return_value {
        assert!(
            vals.contains(&Expr::Literal("http://service".to_string())),
            "try-branch value should be in Joined"
        );
        assert!(
            vals.contains(&Expr::Literal("http://fallback".to_string())),
            "except-branch value should be in Joined"
        );
    } else {
        panic!(
            "Expected Expr::Joined for try/except with differing branches, got {:?}",
            result.return_value
        );
    }
}

// ── Parser: parameter handling ────────────────────────────────────────────────

#[test]
fn python_unresolved_params_produce_concat_of_vars() {
    let code = r#"
def build(self, host: str, path: str) -> str:
    return host + path
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);
    let result = symbolic_evaluation(
        &map,
        "str build(Any,str,str)",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("evaluation should succeed");

    assert!(
        matches!(result.return_value, Expr::Concat(..)),
        "direct concat of two unresolved params should produce Expr::Concat, got {:?}",
        result.return_value
    );
}

// ── Parser: key format ────────────────────────────────────────────────────────

#[test]
fn python_parse_produces_correct_header_key() {
    let code = r#"
def get_url() -> str:
    return "http://svc"
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);

    assert!(
        map.contains_key("str get_url()"),
        "Python function header key should follow format 'ReturnType func_name(TypeList)', map keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
}

#[test]
fn python_parse_header_key_with_typed_params() {
    let code = r#"
def fetch(self, url: str, timeout: int) -> str:
    return url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);

    assert!(
        map.contains_key("str fetch(Any,str,int)"),
        "typed params should be extracted into mangled header, map keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
}
