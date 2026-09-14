use std::collections::HashMap;

use models::{Assignment, AssignmentKey, HttpMethod, Import};
use statix::strings::{normalize_whitespace, strip_quotes};
use tree_sitter::Node;

use crate::extraction::shared::{infer_http_method_from_name, node_text};

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
pub(super) const WEB_IMPORT_PREFIXES: &[&str] = &["github.com/hoisie/web"];

pub(super) fn is_known_router_receiver(
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

pub(super) fn router_prefix(
    function_node: Node,
    code: &str,
    assignments: &HashMap<AssignmentKey, Assignment>,
    imports: &[Import],
) -> Option<String> {
    let origin = receiver_origin(function_node, code, assignments)?;
    router_prefix_from_origin(origin, assignments, imports, &mut Vec::new())
}

pub(super) fn is_serve_mux_receiver(
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

fn router_prefix_from_origin(
    origin: &str,
    assignments: &HashMap<AssignmentKey, Assignment>,
    imports: &[Import],
    seen: &mut Vec<String>,
) -> Option<String> {
    let trimmed = origin.trim();
    if seen.iter().any(|value| value == trimmed) {
        return None;
    }
    seen.push(trimmed.to_string());

    if ROUTER_CONSTRUCTORS
        .iter()
        .any(|constructor| trimmed.starts_with(constructor))
        || imported_origin_matches(trimmed, imports, ROUTER_IMPORT_PREFIXES)
    {
        seen.pop();
        return Some(String::new());
    }

    let resolved = parse_group_origin(trimmed).and_then(|(base, group_prefix)| {
        let base_origin = assignments
            .values()
            .find(|assignment| assignment.variable_name == base)
            .map(|assignment| assignment.value.as_str())
            .unwrap_or(base);
        router_prefix_from_origin(base_origin, assignments, imports, seen)
            .map(|prefix| join_route_prefix(&prefix, &group_prefix))
    });
    seen.pop();
    resolved
}

fn parse_group_origin(origin: &str) -> Option<(&str, String)> {
    let (base, rest) = origin.split_once(".Group(")?;
    let args = rest.strip_suffix(')')?;
    let group_prefix = strip_quotes(args.trim()).to_string();
    Some((base.trim(), group_prefix))
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

pub(super) fn imported_origin_matches(origin: &str, imports: &[Import], packages: &[&str]) -> bool {
    let alias = origin.split('.').next().unwrap_or(origin);
    imports.iter().any(|import| {
        import.module_alias == alias
            && packages
                .iter()
                .any(|package| import.orig_module.starts_with(package))
    })
}

pub(super) fn looks_like_http_path(path: &str) -> bool {
    path.starts_with('/')
}

pub(super) fn infer_method_from_handler(handler: &str) -> HttpMethod {
    infer_http_method_from_name(handler.rsplit('.').next().unwrap_or(handler))
}

pub(super) fn join_route_prefix(prefix: &str, path: &str) -> String {
    match (prefix.is_empty(), path.is_empty()) {
        (true, _) => path.to_string(),
        (_, true) => prefix.to_string(),
        _ => format!(
            "{}/{}",
            prefix.trim_end_matches('/'),
            path.trim_start_matches('/')
        ),
    }
}

pub(super) fn call_arguments(node: Node<'_>) -> Vec<Node<'_>> {
    node.child_by_field_name("arguments")
        .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
        .unwrap_or_default()
}

pub(super) fn normalized_handler(node: Node, code: &str) -> String {
    normalize_whitespace(node_text(node, code))
}

pub(super) fn function_node(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("function")
}
