use std::{collections::HashMap, str::FromStr};

use models::{Assignment, AssignmentKey, Callable, HttpMethod, Scope};
use statix::strings::{normalize_whitespace, strip_quotes};
use tree_sitter::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GoNodeKind {
    InterpretedStringLiteral,
    RawStringLiteral,
    Identifier,
    ParenthesizedExpression,
    BinaryExpression,
    CallExpression,
    SelectorExpression,
    UnaryExpression,
    FunctionDeclaration,
    MethodDeclaration,
    ConstDeclaration,
    VarDeclaration,
    ConstSpec,
    VarSpec,
    ExpressionList,
    ShortVarDeclaration,
    AssignmentStatement,
    Other,
}

impl From<&str> for GoNodeKind {
    fn from(value: &str) -> Self {
        match value {
            "interpreted_string_literal" => Self::InterpretedStringLiteral,
            "raw_string_literal" => Self::RawStringLiteral,
            "identifier" => Self::Identifier,
            "parenthesized_expression" => Self::ParenthesizedExpression,
            "binary_expression" => Self::BinaryExpression,
            "call_expression" => Self::CallExpression,
            "selector_expression" => Self::SelectorExpression,
            "unary_expression" => Self::UnaryExpression,
            "function_declaration" => Self::FunctionDeclaration,
            "method_declaration" => Self::MethodDeclaration,
            "const_declaration" => Self::ConstDeclaration,
            "var_declaration" => Self::VarDeclaration,
            "const_spec" => Self::ConstSpec,
            "var_spec" => Self::VarSpec,
            "expression_list" => Self::ExpressionList,
            "short_var_declaration" => Self::ShortVarDeclaration,
            "assignment_statement" => Self::AssignmentStatement,
            _ => Self::Other,
        }
    }
}

pub(super) fn go_node_kind(node: Node) -> GoNodeKind {
    GoNodeKind::from(node.kind())
}

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
    let mut values = scope_bindings(assignments, &Scope::Global);
    values.extend(scope_bindings(assignments, scope));
    values
}

pub(super) fn evaluate_expression_node(
    node: Node,
    code: &str,
    scope: &HashMap<String, String>,
) -> String {
    match go_node_kind(node) {
        GoNodeKind::InterpretedStringLiteral | GoNodeKind::RawStringLiteral => {
            strip_quotes(node_text(node, code)).to_string()
        }
        GoNodeKind::Identifier => scope
            .get(node_text(node, code))
            .cloned()
            .unwrap_or_else(|| node_text(node, code).to_string()),
        GoNodeKind::ParenthesizedExpression => node
            .named_child(0)
            .map(|child| evaluate_expression_node(child, code, scope))
            .unwrap_or_default(),
        GoNodeKind::BinaryExpression => {
            let children: Vec<Node> = node.named_children(&mut node.walk()).collect();
            if children.len() == 2 && node_text(node, code).contains('+') {
                return evaluate_expression_node(children[0], code, scope)
                    + &evaluate_expression_node(children[1], code, scope);
            }
            normalize_whitespace(node_text(node, code))
        }
        GoNodeKind::CallExpression => evaluate_special_call(node, code, scope)
            .unwrap_or_else(|| normalize_whitespace(node_text(node, code))),
        GoNodeKind::SelectorExpression => scope
            .get(node_text(node, code))
            .cloned()
            .unwrap_or_else(|| normalize_whitespace(node_text(node, code))),
        GoNodeKind::UnaryExpression => node
            .named_child(0)
            .map(|child| evaluate_expression_node(child, code, scope))
            .unwrap_or_else(|| normalize_whitespace(node_text(node, code))),
        GoNodeKind::Other
        | GoNodeKind::FunctionDeclaration
        | GoNodeKind::MethodDeclaration
        | GoNodeKind::ConstDeclaration
        | GoNodeKind::VarDeclaration
        | GoNodeKind::ConstSpec
        | GoNodeKind::VarSpec
        | GoNodeKind::ExpressionList
        | GoNodeKind::ShortVarDeclaration
        | GoNodeKind::AssignmentStatement => normalize_whitespace(node_text(node, code)),
    }
}

pub(super) fn evaluate_expression_text(expr: &str, scope: &HashMap<String, String>) -> String {
    if let Some(value) = scope.get(expr.trim()) {
        return value.clone();
    }

    if let Some(parts) = split_top_level_plus(expr) {
        return parts
            .into_iter()
            .map(|part| evaluate_expression_text(&part, scope))
            .collect::<String>();
    }

    let trimmed = expr.trim().trim_end_matches(',');
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('`') && trimmed.ends_with('`'))
    {
        return strip_quotes(trimmed).to_string();
    }
    if let Some(inner) = trimmed
        .strip_prefix("url.PathEscape(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return format!("{{{}}}", inner.trim());
    }
    if let Some(inner) = trimmed
        .strip_prefix("request.PathValue(")
        .or_else(|| trimmed.strip_prefix("PathValue("))
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return format!("{{{}}}", strip_quotes(inner.trim()));
    }
    if let Some(inner) = trimmed
        .strip_prefix("strings.TrimRight(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let parts = split_argument_text(inner);
        if parts.len() == 2 {
            let base = evaluate_expression_text(parts[0], scope);
            let suffix = strip_quotes(parts[1].trim());
            return base.trim_end_matches(&suffix).to_string();
        }
    }

    trimmed.to_string()
}

pub(super) fn selector_name(node: Node, code: &str) -> Option<String> {
    if go_node_kind(node) == GoNodeKind::Identifier {
        return Some(node_text(node, code).to_string());
    }
    if go_node_kind(node) != GoNodeKind::SelectorExpression {
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

pub(super) fn web_route_method(selector: &str) -> Option<HttpMethod> {
    let method = selector
        .strip_prefix("Get")
        .map(|_| "GET")
        .or_else(|| selector.strip_prefix("Post").map(|_| "POST"))
        .or_else(|| selector.strip_prefix("Put").map(|_| "PUT"))
        .or_else(|| selector.strip_prefix("Delete").map(|_| "DELETE"))?;
    HttpMethod::from_str(method).ok()
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
    match method {
        HttpMethod::GET => "GET",
        HttpMethod::POST => "POST",
        HttpMethod::PUT => "PUT",
        HttpMethod::DELETE => "DELETE",
        HttpMethod::PATCH => "PATCH",
    }
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
