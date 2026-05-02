use models::{Parameter, ir::ast::Expr};

/// Classify a parameter's default value as a simple literal or a complex expression.
///
/// Simple defaults (quoted strings, integers, floats, `True`/`False`/`None`) are
/// safe to use as symbolic values in URI resolution. Quoted strings have their
/// surrounding quotes stripped so `initial_value = "\"aaaa\""` becomes
/// `Expr::Literal("aaaa")`, not `Expr::Literal("\"aaaa\"")`.
///
/// Complex expressions — function calls (`Path(...)`), constructors (`Settings()`),
/// method calls (`api_integration.global_depends()`), lists, dicts — are runtime
/// values that carry no useful static fragment. They are kept as `Expr::Var` so
/// that template placeholders like `{job_id}` remain unresolved in the output URI.
pub(crate) fn param_default_expr(param: &Parameter) -> Expr {
    let Some(raw) = &param.initial_value else {
        return Expr::Var(param.name.clone());
    };
    let s = raw.trim();

    // Quoted string — strip surrounding quotes.
    if s.len() >= 2 {
        let first = s.chars().next().unwrap();
        let last = s.chars().last().unwrap();
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return Expr::Literal(s[1..s.len() - 1].to_string());
        }
    }

    // Boolean / None keywords.
    if matches!(s, "True" | "False" | "None") {
        return Expr::Literal(s.to_string());
    }

    // Numeric literal.
    if s.parse::<f64>().is_ok() {
        return Expr::Literal(s.to_string());
    }

    // Everything else (function calls, constructors, attribute expressions, …)
    // is treated as a runtime value.
    Expr::Var(param.name.clone())
}

/// Decide the datatype of a string-concatenation result.
/// If either operand is typed `String`, the result is `String`.
pub(crate) fn concat_datatype(left: &Option<String>, right: &Option<String>) -> Option<String> {
    if *left == Some("String".to_string()) || *right == Some("String".to_string()) {
        return Some("String".to_string());
    }
    left.clone()
}

/// Flatten a nested `Expr::Attr` chain to a dot-separated key string.
/// `Attr(Attr(Var("a"), "b"), "c")` -> `"a.b.c"`.
pub(crate) fn flatten_attr_to_dot_key(object: &Expr, field: &str) -> String {
    match object {
        Expr::Var(name) => format!("{}.{}", name, field),
        Expr::Attr {
            object: inner_obj,
            field: inner_field,
        } => {
            let base = flatten_attr_to_dot_key(inner_obj, inner_field);
            format!("{}.{}", base, field)
        }
        _ => field.to_string(),
    }
}
