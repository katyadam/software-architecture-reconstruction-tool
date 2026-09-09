use std::collections::HashMap;

use models::ir::ast::Expr;
use statix::{
    parse_python,
    python::matcher::PythonCallableMatcher,
    symbolic_evaluation, symbolic_evaluation_with_env,
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

// ── Closure / cross-module attribute tests ────────────────────────────────────

#[test]
fn attr_parse_settings_as_url_rstrip() {
    // `base_url = settings.as_url.rstrip("/")` should parse to a resolved
    // literal if `settings.as_url` is seeded in the env.
    let code = r#"
def outer(app) -> None:
    base_url = settings.as_url.rstrip("/")
    return base_url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);

    let mut initial_env = HashMap::new();
    initial_env.insert(
        "settings.as_url".to_string(),
        (
            Some("String".to_string()),
            Expr::Literal("http://annotation-service:8000".to_string()),
        ),
    );
    println!("{map:#?}");
    let result = symbolic_evaluation_with_env(
        &map,
        "None outer(Any)",
        Box::new(PythonCallableMatcher::new()),
        &initial_env,
    )
    .expect("evaluation should succeed");

    println!("{result:#?}");

    let base_url = result
        .final_env
        .get("base_url")
        .expect("base_url should be in final_env");

    assert_eq!(
        base_url.1,
        Expr::Literal("http://annotation-service:8000".to_string()),
        "base_url should resolve to the settings.as_url value"
    );
}

#[test]
fn attr_eval_via_dot_key() {
    // Direct attribute lookup via a seeded dot-path key.
    let code = r#"
def get_url() -> str:
    return settings.svc_url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);

    let mut initial_env = HashMap::new();
    initial_env.insert(
        "settings.svc_url".to_string(),
        (
            Some("String".to_string()),
            Expr::Literal("http://service:9000".to_string()),
        ),
    );

    let result = symbolic_evaluation_with_env(
        &map,
        "str get_url()",
        Box::new(PythonCallableMatcher::new()),
        &initial_env,
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://service:9000".to_string()),
        "settings.svc_url should resolve via dot-key lookup"
    );
}

#[test]
fn strip_methods_are_identity_on_receiver() {
    // rstrip/lstrip/strip should return their receiver unchanged.
    let code = r#"
def get_url() -> str:
    raw = settings.svc_url
    trimmed = raw.rstrip("/")
    return trimmed
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);

    let mut initial_env = HashMap::new();
    initial_env.insert(
        "settings.svc_url".to_string(),
        (
            Some("String".to_string()),
            Expr::Literal("http://service:9000/".to_string()),
        ),
    );

    let result = symbolic_evaluation_with_env(
        &map,
        "str get_url()",
        Box::new(PythonCallableMatcher::new()),
        &initial_env,
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("http://service:9000/".to_string()),
        "rstrip should be identity: receiver value is returned unchanged"
    );
}

#[test]
fn nested_callable_refs_recorded_on_outer() {
    // The outer callable's CallableAst.nested should reference the inner function.
    let code = r#"
def add_routes(app) -> None:
    base_url = settings.as_url.rstrip("/")
    async def _(x: int = 0) -> None:
        pass
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);

    let outer_key = "None add_routes(Any)";
    let outer = map
        .get(outer_key)
        .expect("outer callable should be registered");

    assert!(
        !outer.ast.nested.is_empty(),
        "outer callable should record nested ref for inner `_`; got nested={:?}",
        outer.ast.nested
    );

    let inner_key = &outer.ast.nested[0].key;
    assert!(
        map.contains_key(inner_key.as_str()),
        "inner callable should be registered under key {:?}; available keys: {:?}",
        inner_key,
        map.keys().collect::<Vec<_>>()
    );
}

#[test]
fn captured_base_url_resolves_in_nested_callable() {
    // End-to-end: outer callable evaluates `base_url` from settings, nested callable
    // should have `base_url` available when evaluated.
    let code = r#"
def add_routes(app) -> None:
    base_url = settings.as_url.rstrip("/")
    async def inner() -> None:
        url = base_url
        return url
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);

    let mut constants_env: HashMap<String, (Option<String>, Expr)> = HashMap::new();
    constants_env.insert(
        "settings.as_url".to_string(),
        (
            Some("String".to_string()),
            Expr::Literal("http://svc:8000".to_string()),
        ),
    );

    // Evaluate the outer first to get base_url into final_env.
    let outer_result = symbolic_evaluation_with_env(
        &map,
        "None add_routes(Any)",
        Box::new(PythonCallableMatcher::new()),
        &constants_env,
    )
    .expect("outer evaluation should succeed");

    // Collect captured env for the inner callable.
    let outer = map.get("None add_routes(Any)").unwrap();
    let nested_ref = outer.ast.nested.first().expect("should have a nested ref");

    let mut inner_env = constants_env.clone();
    for var_name in &nested_ref.captured {
        if let Some(val) = outer_result.final_env.get(var_name)
            && val.1 != Expr::Empty
        {
            inner_env
                .entry(var_name.clone())
                .or_insert_with(|| val.clone());
        }
    }

    let inner_result = symbolic_evaluation_with_env(
        &map,
        &nested_ref.key,
        Box::new(PythonCallableMatcher::new()),
        &inner_env,
    )
    .expect("inner evaluation should succeed");

    let url = inner_result
        .final_env
        .get("url")
        .expect("url should be in inner final_env after capture");

    assert_eq!(
        url.1,
        Expr::Literal("http://svc:8000".to_string()),
        "captured base_url should propagate into nested callable's env"
    );
}

// ── Additional edge-case tests ────────────────────────────────────────────────

/// Two sibling outer callables each define an inner `_` function with the same
/// name and signature but different bodies. Their mangled keys collide, but their
/// body hashes (stored in `NestedRef::hash`) are distinct — ensuring
/// `build_captured_scopes` can key captured envs by hash without collision.
#[test]
fn two_sibling_outers_with_same_inner_name_have_distinct_hashes() {
    let code = r#"
def outer_a(app) -> None:
    url_a = "http://service-a"
    async def _(x: int = 0) -> None:
        val = url_a
        return val

def outer_b(app) -> None:
    url_b = "http://service-b"
    async def _(x: int = 0) -> None:
        val = url_b
        return val
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);

    let outer_a = map
        .get("None outer_a(Any)")
        .expect("outer_a should be registered");
    let outer_b = map
        .get("None outer_b(Any)")
        .expect("outer_b should be registered");

    assert!(
        !outer_a.ast.nested.is_empty(),
        "outer_a should have nested refs"
    );
    assert!(
        !outer_b.ast.nested.is_empty(),
        "outer_b should have nested refs"
    );

    let key_a = &outer_a.ast.nested[0].key;
    let key_b = &outer_b.ast.nested[0].key;
    let hash_a = &outer_a.ast.nested[0].hash;
    let hash_b = &outer_b.ast.nested[0].hash;

    // Mangled keys still collide (same name + signature).
    assert_eq!(
        key_a, key_b,
        "sibling inner `_` callables share the same mangled key"
    );

    // Body hashes must differ since the bodies reference different variables.
    assert_ne!(
        hash_a, hash_b,
        "sibling inner `_` callables with different bodies must have distinct hashes; \
         hash_a={hash_a:?} hash_b={hash_b:?}"
    );
}

/// `Expr::Attr` chain depth > 2: `a.b.c` should look up "a.b.c" in the env.
#[test]
fn attr_chain_depth_3_resolves_from_env() {
    let code = r#"
def get_url() -> str:
    return config.db.host
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);

    let mut initial_env = HashMap::new();
    initial_env.insert(
        "config.db.host".to_string(),
        (
            Some("String".to_string()),
            Expr::Literal("postgres://localhost".to_string()),
        ),
    );

    let result = symbolic_evaluation_with_env(
        &map,
        "str get_url()",
        Box::new(PythonCallableMatcher::new()),
        &initial_env,
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result.return_value,
        Expr::Literal("postgres://localhost".to_string()),
        "depth-3 attr chain a.b.c should resolve via dot-key lookup"
    );
}

/// `build_captured_scopes` must not pollute unrelated callables.
/// An outer callable that defines no nested functions must not appear
/// in the captured-scopes map; evaluating the unrelated callable with
/// only the constants env must still succeed.
#[test]
fn captured_scope_does_not_pollute_unrelated_callable() {
    let code = r#"
def outer_with_nested(app) -> None:
    secret = "http://internal"
    async def inner() -> None:
        url = secret
        return url

def unrelated() -> str:
    return "http://public"
"#;
    let tree = get_python_tree(code);
    let map = parse_python(&tree, code);

    // Simulate what build_captured_scopes does: evaluate outer_with_nested only.
    let outer = map
        .get("None outer_with_nested(Any)")
        .expect("outer should be registered");
    let nested_ref = outer.ast.nested.first().expect("should have nested ref");

    let constants_env: HashMap<String, (Option<String>, Expr)> = HashMap::new();

    let outer_result = symbolic_evaluation_with_env(
        &map,
        "None outer_with_nested(Any)",
        Box::new(PythonCallableMatcher::new()),
        &constants_env,
    )
    .expect("outer evaluation should succeed");

    // Build captured scope only for the inner callable.
    let mut captured_scopes: HashMap<String, HashMap<String, (Option<String>, Expr)>> =
        HashMap::new();
    let captured: HashMap<_, _> = nested_ref
        .captured
        .iter()
        .filter_map(|var| {
            outer_result
                .final_env
                .get(var)
                .map(|v| (var.clone(), v.clone()))
        })
        .filter(|(_, (_, expr))| *expr != Expr::Empty)
        .collect();
    if !captured.is_empty() {
        captured_scopes.insert(nested_ref.key.clone(), captured);
    }

    // The unrelated callable must NOT appear in captured_scopes.
    let unrelated_result = symbolic_evaluation_with_env(
        &map,
        "str unrelated()",
        Box::new(PythonCallableMatcher::new()),
        &constants_env,
    )
    .expect("unrelated evaluation should succeed");

    // Verify captured_scopes has no entry for unrelated().
    // We check that the unrelated callable's mangled key is absent.
    let unrelated_mangled = "str unrelated()";
    assert!(
        !captured_scopes.contains_key(unrelated_mangled),
        "captured_scopes must not contain an entry for the unrelated callable"
    );

    // And that unrelated() still evaluates correctly.
    assert_eq!(
        unrelated_result.return_value,
        Expr::Literal("http://public".to_string()),
        "unrelated callable should return its own literal, unaffected by captured scopes"
    );

    // Also confirm the inner callable's captured scope contains `secret`.
    let inner_captured = captured_scopes
        .get(&nested_ref.key)
        .expect("inner callable should have a captured scope");
    assert!(
        inner_captured.contains_key("secret"),
        "captured scope for inner should contain `secret`"
    );
}
