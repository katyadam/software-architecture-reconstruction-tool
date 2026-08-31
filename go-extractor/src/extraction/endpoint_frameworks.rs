use std::collections::HashMap;

use models::{Assignment, AssignmentKey, HttpMethod, Import};
use statix::strings::{normalize_whitespace, strip_quotes};
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
const WEB_IMPORT_PREFIXES: &[&str] = &["github.com/hoisie/web"];

pub(super) struct EndpointMatch {
    pub method: HttpMethod,
    pub path: String,
    pub handler: String,
}

pub(super) struct ExtractParams<'a> {
    pub node: Node<'a>,
    pub code: &'a str,
    pub globals: &'a HashMap<String, String>,
    pub assignments: &'a HashMap<AssignmentKey, Assignment>,
    pub imports: &'a [Import],
}

pub(super) trait EndpointIdentificationStrategy: Sync {
    fn identify(&self, params: &ExtractParams<'_>) -> Option<EndpointMatch>;
}

pub(super) fn strategies() -> &'static [&'static dyn EndpointIdentificationStrategy] {
    STRATEGIES
}

struct GorillaStrategy;
struct ChiStrategy;
struct ServeMuxHandleStrategy;
struct ServeMuxHandleFuncStrategy;
struct WebStrategy;

static GORILLA: GorillaStrategy = GorillaStrategy;
static CHI: ChiStrategy = ChiStrategy;
static SERVE_MUX_HANDLE: ServeMuxHandleStrategy = ServeMuxHandleStrategy;
static SERVE_MUX_HANDLE_FUNC: ServeMuxHandleFuncStrategy = ServeMuxHandleFuncStrategy;
static WEB: WebStrategy = WebStrategy;
static STRATEGIES: &[&dyn EndpointIdentificationStrategy] = &[
    &GORILLA,
    &CHI,
    &SERVE_MUX_HANDLE,
    &SERVE_MUX_HANDLE_FUNC,
    &WEB,
];

impl EndpointIdentificationStrategy for GorillaStrategy {
    fn identify(&self, params: &ExtractParams<'_>) -> Option<EndpointMatch> {
        let function_node = params.node.child_by_field_name("function")?;
        if selector_name(function_node, params.code)? != "Methods" {
            return None;
        }

        let selector_operand = function_node.child_by_field_name("operand")?;
        if selector_operand.kind() != "call_expression" {
            return None;
        }

        let inner_function = selector_operand.child_by_field_name("function")?;
        if selector_name(inner_function, params.code)? != "HandleFunc" {
            return None;
        }

        let inner_args = selector_operand
            .child_by_field_name("arguments")
            .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())?;
        let method_args = params
            .node
            .child_by_field_name("arguments")
            .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())?;
        if inner_args.len() < 2 || method_args.is_empty() {
            return None;
        }

        Some(EndpointMatch {
            method: parse_http_method_value(&evaluate_expression_node(
                method_args[0],
                params.code,
                params.globals,
            ))?,
            path: evaluate_expression_node(inner_args[0], params.code, params.globals),
            handler: normalize_whitespace(node_text(inner_args[1], params.code)),
        })
    }
}

impl EndpointIdentificationStrategy for ChiStrategy {
    fn identify(&self, params: &ExtractParams<'_>) -> Option<EndpointMatch> {
        let function_node = params.node.child_by_field_name("function")?;
        let prefix = router_prefix(
            function_node,
            params.code,
            params.assignments,
            params.imports,
        )?;
        if prefix.is_empty()
            && !is_known_router_receiver(
                function_node,
                params.code,
                params.assignments,
                params.imports,
            )
        {
            return None;
        }
        let selector = selector_name(function_node, params.code)?;
        let arguments = params
            .node
            .child_by_field_name("arguments")
            .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
            .unwrap_or_default();

        match selector.as_str() {
            "Get" | "Post" | "Put" | "Delete" | "Patch" | "Options" | "Head" => {
                if arguments.len() < 2 {
                    return None;
                }
                let path = join_route_prefix(
                    &prefix,
                    &evaluate_expression_node(arguments[0], params.code, params.globals),
                );
                if !looks_like_http_path(&path) {
                    return None;
                }
                Some(EndpointMatch {
                    method: parse_http_method_value(selector.as_str())?,
                    path,
                    handler: normalize_whitespace(node_text(arguments[1], params.code)),
                })
            }
            _ if is_http_method_selector(selector.as_str()) => {
                if arguments.len() < 2 {
                    return None;
                }
                let path = join_route_prefix(
                    &prefix,
                    &evaluate_expression_node(arguments[0], params.code, params.globals),
                );
                if !looks_like_http_path(&path) {
                    return None;
                }
                Some(EndpointMatch {
                    method: parse_http_method_value(selector.as_str())?,
                    path,
                    handler: normalize_whitespace(node_text(arguments[1], params.code)),
                })
            }
            "Method" | "MethodFunc" => {
                if arguments.len() < 3 {
                    return None;
                }
                let path = join_route_prefix(
                    &prefix,
                    &evaluate_expression_node(arguments[1], params.code, params.globals),
                );
                if !looks_like_http_path(&path) {
                    return None;
                }
                Some(EndpointMatch {
                    method: parse_http_method_value(&evaluate_expression_node(
                        arguments[0],
                        params.code,
                        params.globals,
                    ))?,
                    path,
                    handler: normalize_whitespace(node_text(arguments[2], params.code)),
                })
            }
            _ => None,
        }
    }
}

impl EndpointIdentificationStrategy for ServeMuxHandleStrategy {
    fn identify(&self, params: &ExtractParams<'_>) -> Option<EndpointMatch> {
        let function_node = params.node.child_by_field_name("function")?;
        if selector_name(function_node, params.code)? != "Handle" {
            return None;
        }
        if !is_serve_mux_receiver(
            function_node,
            params.code,
            params.assignments,
            params.imports,
        ) {
            return None;
        }

        let arguments = params
            .node
            .child_by_field_name("arguments")
            .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
            .unwrap_or_default();
        if arguments.len() < 2 {
            return None;
        }

        let path = evaluate_expression_node(arguments[0], params.code, params.globals);
        if !looks_like_http_path(&path) {
            return None;
        }

        Some(EndpointMatch {
            method: infer_method_from_handler(&normalize_whitespace(node_text(
                arguments[1],
                params.code,
            ))),
            path,
            handler: normalize_whitespace(node_text(arguments[1], params.code)),
        })
    }
}

impl EndpointIdentificationStrategy for ServeMuxHandleFuncStrategy {
    fn identify(&self, params: &ExtractParams<'_>) -> Option<EndpointMatch> {
        let function_node = params.node.child_by_field_name("function")?;
        if selector_name(function_node, params.code)? != "HandleFunc" {
            return None;
        }
        if !is_serve_mux_receiver(
            function_node,
            params.code,
            params.assignments,
            params.imports,
        ) {
            return None;
        }

        let arguments = params
            .node
            .child_by_field_name("arguments")
            .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
            .unwrap_or_default();
        if arguments.len() < 2 {
            return None;
        }

        let resolved = evaluate_expression_node(arguments[0], params.code, params.globals);
        let (method, path) = split_method_and_path(&resolved)?;
        Some(EndpointMatch {
            method,
            path,
            handler: normalize_whitespace(node_text(arguments[1], params.code)),
        })
    }
}

impl EndpointIdentificationStrategy for WebStrategy {
    fn identify(&self, params: &ExtractParams<'_>) -> Option<EndpointMatch> {
        let function_node = params.node.child_by_field_name("function")?;
        if !is_web_route_call(function_node, params.code, params.imports) {
            return None;
        }

        let selector = selector_name(function_node, params.code)?;
        let method = web_route_method(&selector)?;
        let arguments = params
            .node
            .child_by_field_name("arguments")
            .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
            .unwrap_or_default();
        if arguments.len() < 2 {
            return None;
        }

        Some(EndpointMatch {
            method,
            path: evaluate_expression_node(arguments[0], params.code, params.globals),
            handler: normalize_whitespace(node_text(arguments[1], params.code)),
        })
    }
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

fn router_prefix(
    function_node: Node,
    code: &str,
    assignments: &HashMap<AssignmentKey, Assignment>,
    imports: &[Import],
) -> Option<String> {
    let origin = receiver_origin(function_node, code, assignments)?;
    router_prefix_from_origin(origin, assignments, imports, &mut Vec::new())
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
}

fn infer_method_from_handler(handler: &str) -> HttpMethod {
    infer_http_method_from_name(handler.rsplit('.').next().unwrap_or(handler))
}

fn is_web_route_call(function_node: Node, code: &str, imports: &[Import]) -> bool {
    let Some(selector) = selector_name(function_node, code) else {
        return false;
    };
    if web_route_method(&selector).is_none() {
        return false;
    }
    let Some(receiver) = function_node.child_by_field_name("operand") else {
        return false;
    };
    let receiver = node_text(receiver, code).trim();
    imports.iter().any(|import| {
        import.module_alias == receiver
            && WEB_IMPORT_PREFIXES
                .iter()
                .any(|prefix| import.orig_module.starts_with(prefix))
    })
}

fn join_route_prefix(prefix: &str, path: &str) -> String {
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
