use std::collections::HashMap;

use models::{Assignment, AssignmentKey, HttpMethod, Import};
use statix::strings::normalize_whitespace;
use tree_sitter::Node;

use super::shared::{
    evaluate_expression_node, infer_http_method_from_name, is_http_method_selector, node_text,
    parse_http_method_value, selector_name, split_method_and_path, web_route_method,
};

const ROUTER_CONSTRUCTORS: &[&str] = &[
    "chi.NewRouter(",
    "gin.Default(",
    "gin.New(",
    "echo.New(",
    "fiber.New(",
    "httprouter.New(",
];
const ROUTER_IMPORT_PREFIXES: &[&str] = &[
    "github.com/go-chi/chi",
    "github.com/gin-gonic/gin",
    "github.com/labstack/echo",
    "github.com/gofiber/fiber",
    "github.com/julienschmidt/httprouter",
];
const SERVE_MUX_IMPORT_PREFIXES: &[&str] = &["net/http"];

pub(super) fn gorilla_route_parts(
    node: Node,
    code: &str,
    globals: &HashMap<String, String>,
) -> Option<(HttpMethod, String, String)> {
    let function_node = node.child_by_field_name("function")?;
    if selector_name(function_node, code)? != "Methods" {
        return None;
    }

    let selector_operand = function_node.child_by_field_name("operand")?;
    if selector_operand.kind() != "call_expression" {
        return None;
    }

    let inner_function = selector_operand.child_by_field_name("function")?;
    if selector_name(inner_function, code)? != "HandleFunc" {
        return None;
    }

    let inner_args = selector_operand
        .child_by_field_name("arguments")
        .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())?;
    let method_args = node
        .child_by_field_name("arguments")
        .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())?;
    if inner_args.len() < 2 || method_args.is_empty() {
        return None;
    }

    let method = parse_http_method_value(&evaluate_expression_node(method_args[0], code, globals))?;
    let path = evaluate_expression_node(inner_args[0], code, globals);
    let handler = normalize_whitespace(node_text(inner_args[1], code));
    Some((method, path, handler))
}

pub(super) fn chi_route_parts(
    node: Node,
    code: &str,
    globals: &HashMap<String, String>,
    assignments: &HashMap<AssignmentKey, Assignment>,
    imports: &[Import],
) -> Option<(HttpMethod, String, String)> {
    let function_node = node.child_by_field_name("function")?;
    if !is_known_router_receiver(function_node, code, assignments, imports) {
        return None;
    }
    let selector = selector_name(function_node, code)?;
    let arguments = node
        .child_by_field_name("arguments")
        .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
        .unwrap_or_default();

    match selector.as_str() {
        "Get" | "Post" | "Put" | "Delete" | "Patch" | "Options" | "Head" => {
            if arguments.len() < 2 {
                return None;
            }
            let path = evaluate_expression_node(arguments[0], code, globals);
            if !looks_like_http_path(&path) {
                return None;
            }
            let method = parse_http_method_value(selector.as_str())?;
            let handler = normalize_whitespace(node_text(arguments[1], code));
            Some((method, path, handler))
        }
        _ if is_http_method_selector(selector.as_str()) => {
            if arguments.len() < 2 {
                return None;
            }
            let path = evaluate_expression_node(arguments[0], code, globals);
            if !looks_like_http_path(&path) {
                return None;
            }
            let method = parse_http_method_value(selector.as_str())?;
            let handler = normalize_whitespace(node_text(arguments[1], code));
            Some((method, path, handler))
        }
        "Method" | "MethodFunc" => {
            if arguments.len() < 3 {
                return None;
            }
            let method =
                parse_http_method_value(&evaluate_expression_node(arguments[0], code, globals))?;
            let path = evaluate_expression_node(arguments[1], code, globals);
            if !looks_like_http_path(&path) {
                return None;
            }
            let handler = normalize_whitespace(node_text(arguments[2], code));
            Some((method, path, handler))
        }
        _ => None,
    }
}

pub(super) fn serve_mux_route_parts(
    node: Node,
    code: &str,
    globals: &HashMap<String, String>,
    assignments: &HashMap<AssignmentKey, Assignment>,
    imports: &[Import],
) -> Option<(HttpMethod, String, String)> {
    let function_node = node.child_by_field_name("function")?;
    if selector_name(function_node, code)? != "Handle" {
        return None;
    }
    if !is_serve_mux_receiver(function_node, code, assignments, imports) {
        return None;
    }

    let arguments = node
        .child_by_field_name("arguments")
        .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
        .unwrap_or_default();
    if arguments.len() < 2 {
        return None;
    }

    let path = evaluate_expression_node(arguments[0], code, globals);
    if !looks_like_http_path(&path) {
        return None;
    }

    let handler = normalize_whitespace(node_text(arguments[1], code));
    Some((infer_method_from_handler(&handler), path, handler))
}

pub(super) fn serve_mux_handle_func_parts(
    node: Node,
    code: &str,
    globals: &HashMap<String, String>,
    assignments: &HashMap<AssignmentKey, Assignment>,
    imports: &[Import],
) -> Option<(HttpMethod, String, String)> {
    let function_node = node.child_by_field_name("function")?;
    if selector_name(function_node, code)? != "HandleFunc" {
        return None;
    }
    if !is_serve_mux_receiver(function_node, code, assignments, imports) {
        return None;
    }

    let arguments = node
        .child_by_field_name("arguments")
        .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
        .unwrap_or_default();
    if arguments.len() < 2 {
        return None;
    }

    let resolved = evaluate_expression_node(arguments[0], code, globals);
    let (method, uri) = split_method_and_path(&resolved)?;
    let handler = normalize_whitespace(node_text(arguments[1], code));
    Some((method, uri, handler))
}

pub(super) fn web_route_parts(
    node: Node,
    code: &str,
    globals: &HashMap<String, String>,
) -> Option<(HttpMethod, String, String)> {
    let function_node = node.child_by_field_name("function")?;
    if !is_web_route_call(function_node, code) {
        return None;
    }

    let selector = selector_name(function_node, code)?;
    let method = web_route_method(&selector)?;
    let arguments = node
        .child_by_field_name("arguments")
        .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
        .unwrap_or_default();
    if arguments.len() < 2 {
        return None;
    }

    let uri = evaluate_expression_node(arguments[0], code, globals);
    let handler = normalize_whitespace(node_text(arguments[1], code));
    Some((method, uri, handler))
}

fn is_known_router_receiver(
    function_node: Node,
    code: &str,
    assignments: &HashMap<AssignmentKey, Assignment>,
    imports: &[Import],
) -> bool {
    receiver_origin(function_node, code, assignments).is_some_and(|origin| {
        ROUTER_CONSTRUCTORS
            .iter()
            .any(|constructor| origin.starts_with(constructor))
            || imported_origin_matches(origin, imports, ROUTER_IMPORT_PREFIXES)
    })
}

fn is_serve_mux_receiver(
    function_node: Node,
    code: &str,
    assignments: &HashMap<AssignmentKey, Assignment>,
    imports: &[Import],
) -> bool {
    receiver_origin(function_node, code, assignments).is_some_and(|origin| {
        origin.starts_with("http.NewServeMux(")
            || origin == "http.DefaultServeMux"
            || imported_origin_matches(origin, imports, SERVE_MUX_IMPORT_PREFIXES)
    })
}

fn receiver_origin<'a>(
    function_node: Node,
    code: &'a str,
    assignments: &'a HashMap<AssignmentKey, Assignment>,
) -> Option<&'a str> {
    let receiver = function_node.child_by_field_name("operand")?;
    let receiver = node_text(receiver, code).trim();
    let mut values = assignments
        .values()
        .filter(|assignment| assignment.variable_name == receiver)
        .map(|assignment| assignment.value.as_str());
    let first = values.next().unwrap_or(receiver);
    if values.any(|value| value != first) {
        return None;
    }
    Some(first)
}

fn imported_origin_matches(origin: &str, imports: &[Import], packages: &[&str]) -> bool {
    let alias = origin.split('.').next().unwrap_or(origin);
    imports.iter().any(|import| {
        import.module_alias == alias
            && packages
                .iter()
                .any(|package| import.orig_module.starts_with(package))
    })
}

fn looks_like_http_path(path: &str) -> bool {
    path.starts_with('/')
        || path.starts_with("./")
        || path.starts_with("../")
        || path.starts_with('{')
        || path.contains("/{")
}

fn infer_method_from_handler(handler: &str) -> HttpMethod {
    let normalized = handler
        .trim()
        .trim_end_matches("()")
        .rsplit('.')
        .next()
        .unwrap_or(handler);
    infer_http_method_from_name(normalized)
}

fn is_web_route_call(function_node: Node, code: &str) -> bool {
    if function_node.kind() != "selector_expression" {
        return false;
    }

    function_node
        .child_by_field_name("operand")
        .map(|operand| normalize_whitespace(node_text(operand, code)) == "web")
        .unwrap_or(false)
}
