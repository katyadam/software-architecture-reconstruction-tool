use std::{
    collections::{HashMap, HashSet},
    path::Path,
    str::FromStr,
};

use models::{Assignment, AssignmentKey, Callable, HttpMethod, Scope};
use statix::strings::{normalize_whitespace, strip_quotes};
use tree_sitter::Node;

const ROUTE_METHOD_PREFIXES: &[(&str, HttpMethod)] = &[
    ("Get", HttpMethod::GET),
    ("Post", HttpMethod::POST),
    ("Put", HttpMethod::PUT),
    ("Delete", HttpMethod::DELETE),
];
const HANDLER_METHOD_HINTS: &[(HttpMethod, &[&str])] = &[
    (HttpMethod::POST, &["Post", "Create", "Add"]),
    (HttpMethod::PUT, &["Put", "Update"]),
    (HttpMethod::DELETE, &["Delete", "Remove"]),
    (HttpMethod::PATCH, &["Patch"]),
];
pub(super) const SYNTHETIC_HANDLER_PREFIX: &str = "handler ";

pub(super) fn scope_bindings(
    assignments: &HashMap<AssignmentKey, Assignment>,
    scope: &Scope,
) -> HashMap<String, String> {
    assignments
        .iter()
        .filter(|(key, _)| key.scope == *scope)
        .map(|(_, assignment)| (assignment.variable_name.clone(), assignment.value.clone()))
        .collect()
}

pub(super) fn merged_scope_bindings(
    assignments: &HashMap<AssignmentKey, Assignment>,
    scope: &Scope,
) -> HashMap<String, String> {
    merged_scope_bindings_with_globals(
        assignments,
        scope,
        &scope_bindings(assignments, &Scope::Global),
    )
}

pub(super) fn merged_scope_bindings_with_globals(
    assignments: &HashMap<AssignmentKey, Assignment>,
    scope: &Scope,
    globals: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut values = globals.clone();
    values.extend(scope_bindings(assignments, scope));
    values
}

pub(super) fn evaluate_expression_node(
    node: Node,
    code: &str,
    scope: &HashMap<String, String>,
) -> String {
    match node.kind() {
        "interpreted_string_literal" | "raw_string_literal" => {
            strip_quotes(node_text(node, code)).to_string()
        }
        "identifier" => resolve_scope_value(node_text(node, code), scope)
            .map(|value| resolve_bound_value(node_text(node, code), value, scope))
            .unwrap_or_else(|| node_text(node, code).to_string()),
        "parenthesized_expression" => node
            .named_child(0)
            .map(|child| evaluate_expression_node(child, code, scope))
            .unwrap_or_default(),
        "binary_expression" => {
            let children: Vec<Node> = node.named_children(&mut node.walk()).collect();
            if children.len() == 2 && node_text(node, code).contains('+') {
                return evaluate_expression_node(children[0], code, scope)
                    + &evaluate_expression_node(children[1], code, scope);
            }
            normalize_whitespace(node_text(node, code))
        }
        "call_expression" => evaluate_special_call(node, code, scope)
            .unwrap_or_else(|| normalize_whitespace(node_text(node, code))),
        "selector_expression" => resolve_scope_value(node_text(node, code), scope)
            .map(|value| resolve_bound_value(node_text(node, code), value, scope))
            .unwrap_or_else(|| normalize_whitespace(node_text(node, code))),
        "unary_expression" => node
            .named_child(0)
            .map(|child| evaluate_expression_node(child, code, scope))
            .unwrap_or_else(|| normalize_whitespace(node_text(node, code))),
        _ => normalize_whitespace(node_text(node, code)),
    }
}

pub(super) fn evaluate_expression_text(expr: &str, scope: &HashMap<String, String>) -> String {
    evaluate_expression_text_inner(expr, scope, &mut HashSet::new())
}

pub(super) fn selector_name(node: Node, code: &str) -> Option<String> {
    if node.kind() == "identifier" {
        return Some(node_text(node, code).to_string());
    }
    if node.kind() != "selector_expression" {
        return None;
    }
    node.child_by_field_name("field")
        .map(|field| node_text(field, code).to_string())
        .or_else(|| {
            node.named_children(&mut node.walk())
                .last()
                .map(|field| node_text(field, code).to_string())
        })
}

pub(super) fn is_http_method_selector(selector: &str) -> bool {
    HttpMethod::ALL
        .iter()
        .any(|method| method.as_str() == selector)
}

pub(super) fn web_route_method(selector: &str) -> Option<HttpMethod> {
    ROUTE_METHOD_PREFIXES
        .iter()
        .find_map(|(prefix, method)| selector.strip_prefix(prefix).map(|_| *method))
}

pub(super) fn parse_http_method_value(value: &str) -> Option<HttpMethod> {
    let trimmed = value.trim();
    let normalized = trimmed
        .strip_prefix("http.Method")
        .unwrap_or(trimmed)
        .trim_matches('"')
        .trim_matches('`');
    HttpMethod::from_str(&normalized.to_uppercase()).ok()
}

pub(super) fn split_method_and_path(value: &str) -> Option<(HttpMethod, String)> {
    let (method, path) = value.split_once(' ')?;
    Some((HttpMethod::from_str(method).ok()?, path.to_string()))
}

pub(super) fn format_http_method(method: &HttpMethod) -> &'static str {
    method.as_str()
}

pub(super) fn infer_http_method_from_name(name: &str) -> HttpMethod {
    HANDLER_METHOD_HINTS
        .iter()
        .find_map(|(method, hints)| contains_any(name, hints).then_some(*method))
        .unwrap_or(HttpMethod::GET)
}

pub(super) fn simple_callable_name(signature: &str) -> Option<String> {
    signature
        .split('(')
        .next()
        .map(str::trim)
        .and_then(|prefix| prefix.split_whitespace().next_back())
        .map(|name| name.to_string())
}

pub(super) fn lookup_callable(
    handler_expr: &str,
    callable_lookup: &HashMap<String, Callable>,
) -> Option<Callable> {
    let trimmed = handler_expr.trim();
    let simple = trimmed.split('.').next_back().unwrap_or(trimmed);
    callable_lookup
        .get(trimmed)
        .cloned()
        .or_else(|| callable_lookup.get(simple).cloned())
}

pub(super) fn walk_named(node: Node, visit: &mut impl FnMut(Node)) {
    visit(node);
    for child in node.named_children(&mut node.walk()) {
        walk_named(child, visit);
    }
}

pub(super) fn node_text<'a>(node: Node, code: &'a str) -> &'a str {
    &code[node.start_byte()..node.end_byte()]
}

pub(super) fn package_path(file_path: &str) -> String {
    Path::new(file_path)
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn contains_any(value: &str, hints: &[&str]) -> bool {
    hints.iter().any(|hint| value.contains(hint))
}

fn resolve_scope_value<'a>(expr: &str, scope: &'a HashMap<String, String>) -> Option<&'a String> {
    if let Some(value) = scope.get(expr.trim()) {
        return Some(value);
    }

    let field = expr.trim().split('.').next_back()?;
    let mut matches = scope
        .iter()
        .filter(|(key, _)| key.ends_with(&format!(".{field}")))
        .map(|(_, value)| value);
    let value = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(value)
}

fn resolve_bound_value(expr: &str, value: &str, scope: &HashMap<String, String>) -> String {
    if value.trim() == expr.trim() {
        return value.to_string();
    }
    evaluate_expression_text(value, scope)
}

fn substitute_scope_tokens(expr: &str, scope: &HashMap<String, String>) -> String {
    let mut result = String::new();
    let mut token = String::new();

    for ch in expr.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.') {
            token.push(ch);
            continue;
        }

        flush_scope_token(&mut result, &mut token, scope);
        result.push(ch);
    }

    flush_scope_token(&mut result, &mut token, scope);
    result
}

fn flush_scope_token(result: &mut String, token: &mut String, scope: &HashMap<String, String>) {
    if token.is_empty() {
        return;
    }

    if token.contains('.')
        && let Some(value) = resolve_scope_value(token, scope)
        && value.trim() != token.trim()
    {
        result.push_str(&evaluate_expression_text(value, scope));
    } else {
        result.push_str(token);
    }
    token.clear();
}

fn evaluate_expression_text_inner(
    expr: &str,
    scope: &HashMap<String, String>,
    seen: &mut HashSet<String>,
) -> String {
    let trimmed = expr.trim().trim_end_matches(',');
    if !seen.insert(trimmed.to_string()) {
        return trimmed.to_string();
    }

    let resolved = if let Some(value) = resolve_scope_value(trimmed, scope) {
        if value.trim() == trimmed {
            value.clone()
        } else {
            evaluate_expression_text_inner(value, scope, seen)
        }
    } else if let Some(parts) = split_top_level_plus(trimmed) {
        parts
            .into_iter()
            .map(|part| evaluate_expression_text_inner(&part, scope, seen))
            .collect::<String>()
    } else if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('`') && trimmed.ends_with('`'))
    {
        strip_quotes(trimmed).to_string()
    } else if let Some(inner) = trimmed
        .strip_prefix("url.PathEscape(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        format!("{{{}}}", inner.trim())
    } else if let Some(inner) = trimmed
        .strip_prefix("request.PathValue(")
        .or_else(|| trimmed.strip_prefix("PathValue("))
        .and_then(|rest| rest.strip_suffix(')'))
    {
        format!("{{{}}}", strip_quotes(inner.trim()))
    } else if let Some(inner) = trimmed
        .strip_prefix("strings.TrimRight(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let parts = split_argument_text(inner);
        if parts.len() == 2 {
            let base = evaluate_expression_text_inner(parts[0], scope, seen);
            let suffix = strip_quotes(parts[1].trim());
            base.trim_end_matches(&suffix).to_string()
        } else {
            trimmed.to_string()
        }
    } else if let Some(inner) = trimmed
        .strip_prefix("fmt.Sprintf(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let parts = split_argument_text(inner);
        if let Some((format, args)) = parts.split_first() {
            let format = evaluate_expression_text_inner(format, scope, seen);
            let args = args
                .iter()
                .map(|arg| evaluate_expression_text_inner(arg, scope, seen))
                .collect::<Vec<_>>();
            apply_sprintf(&format, &args)
        } else {
            trimmed.to_string()
        }
    } else if let Some((map_name, key)) = parse_index_expression(trimmed) {
        let key = evaluate_expression_text_inner(key, scope, seen);
        scope
            .get(&format!("{map_name}[{key}]"))
            .cloned()
            .unwrap_or_else(|| trimmed.to_string())
    } else {
        let substituted = substitute_scope_tokens(trimmed, scope);
        if substituted != trimmed {
            evaluate_expression_text_inner(&substituted, scope, seen)
        } else {
            trimmed.to_string()
        }
    };

    seen.remove(trimmed);
    resolved
}

fn evaluate_special_call(
    node: Node,
    code: &str,
    scope: &HashMap<String, String>,
) -> Option<String> {
    let function_node = node.child_by_field_name("function")?;
    let selector = selector_name(function_node, code)?;
    let arguments = node
        .child_by_field_name("arguments")
        .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
        .unwrap_or_default();

    match selector.as_str() {
        "PathEscape" => arguments.first().map(|arg| {
            let value = node_text(*arg, code).trim().to_string();
            format!("{{{value}}}")
        }),
        "PathValue" => arguments.first().map(|arg| {
            let value = strip_quotes(node_text(*arg, code));
            format!("{{{value}}}")
        }),
        "TrimRight" => {
            if arguments.len() < 2 {
                return None;
            }
            let base = evaluate_expression_node(arguments[0], code, scope);
            let suffix = strip_quotes(node_text(arguments[1], code));
            Some(base.trim_end_matches(&suffix).to_string())
        }
        "Sprintf" => {
            let format = arguments
                .first()
                .map(|arg| evaluate_expression_node(*arg, code, scope))?;
            let args = arguments
                .iter()
                .skip(1)
                .map(|arg| evaluate_expression_node(*arg, code, scope))
                .collect::<Vec<_>>();
            Some(apply_sprintf(&format, &args))
        }
        _ => None,
    }
}

fn split_argument_text(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut quote = '\0';
    let mut start = 0usize;

    for (index, ch) in input.char_indices() {
        match ch {
            '"' | '`' => {
                if !in_string {
                    in_string = true;
                    quote = ch;
                } else if ch == quote {
                    in_string = false;
                }
            }
            '(' if !in_string => depth += 1,
            ')' if !in_string => depth -= 1,
            ',' if !in_string && depth == 0 => {
                parts.push(input[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }

    let tail = input[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

pub(super) fn apply_sprintf(format: &str, args: &[String]) -> String {
    let mut result = String::new();
    let mut chars = format.chars().peekable();
    let mut arg_index = 0usize;

    while let Some(ch) = chars.next() {
        if ch != '%' {
            result.push(ch);
            continue;
        }

        if chars.peek().is_some_and(|next| *next == '%') {
            chars.next();
            result.push('%');
            continue;
        }

        let mut verb = None;
        while let Some(next) = chars.next() {
            if next.is_ascii_alphabetic() {
                verb = Some(next);
                break;
            }
        }

        if matches!(verb, Some('s' | 'd' | 'v' | 'q')) {
            let value = args.get(arg_index).cloned().unwrap_or_default();
            result.push_str(&strip_quotes(value.trim()));
            arg_index += 1;
        }
    }

    result
}

fn parse_index_expression(expr: &str) -> Option<(&str, &str)> {
    let open = expr.find('[')?;
    let close = expr.rfind(']')?;
    if close <= open + 1 {
        return None;
    }
    Some((expr[..open].trim(), expr[open + 1..close].trim()))
}

fn split_top_level_plus(input: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut quote = '\0';
    let mut start = 0usize;
    let mut seen_plus = false;

    for (index, ch) in input.char_indices() {
        match ch {
            '"' | '`' => {
                if !in_string {
                    in_string = true;
                    quote = ch;
                } else if ch == quote {
                    in_string = false;
                }
            }
            '(' if !in_string => depth += 1,
            ')' if !in_string => depth -= 1,
            '+' if !in_string && depth == 0 => {
                parts.push(input[start..index].trim().to_string());
                start = index + 1;
                seen_plus = true;
            }
            _ => {}
        }
    }

    if !seen_plus {
        return None;
    }
    parts.push(input[start..].trim().to_string());
    Some(parts)
}
