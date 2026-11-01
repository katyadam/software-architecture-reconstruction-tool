use std::collections::HashMap;

use models::{Assignment, AssignmentKey, HttpMethod, Import};
use statix::strings::normalize_whitespace;
use tree_sitter::Node;

use super::ExtractParams;
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
/// Router types by package, for parameters typed `<alias>.<Type>` (allowlist: `*gin.Context` is no router).
pub(super) const ROUTER_PARAMETER_TYPES: &[(&str, &[&str])] = &[
    ("github.com/go-chi/chi", &["Router", "Mux"]),
    (
        "github.com/gin-gonic/gin",
        &["Engine", "RouterGroup", "IRouter", "IRoutes"],
    ),
    ("github.com/labstack/echo", &["Echo", "Group"]),
    ("github.com/gofiber/fiber", &["App", "Router"]),
    ("github.com/julienschmidt/httprouter", &["Router"]),
    ("github.com/gorilla/mux", &["Router"]),
];
pub(super) const SERVE_MUX_PARAMETER_TYPES: &[(&str, &[&str])] = &[("net/http", &["ServeMux"])];
pub(super) const WEB_IMPORT_PREFIXES: &[&str] = &["github.com/hoisie/web"];

/// Receiver is a known router, or a `.Group(` / `.PathPrefix(..).Subrouter(` chain of one.
/// The group prefix itself comes from the route-fact resolver.
pub(super) fn is_known_router_receiver(
    function_node: Node,
    code: &str,
    assignments: &HashMap<AssignmentKey, Assignment>,
    imports: &[Import],
) -> bool {
    receiver_origin(function_node, code, assignments)
        .is_some_and(|origin| is_router_origin(origin, assignments, imports, &mut Vec::new()))
}

/// Receiver is a parameter of the enclosing named function whose type is in `types`.
/// Closure (`func_literal`) parameters are not accepted.
pub(super) fn is_router_parameter(
    function_node: Node,
    params: &ExtractParams<'_>,
    types: &[(&str, &[&str])],
) -> bool {
    let code = params.code;
    let Some(receiver) = function_node
        .child_by_field_name("operand")
        .filter(|operand| operand.kind() == "identifier")
    else {
        return false;
    };
    let receiver = node_text(receiver, code);
    // ponytail: nested-block shadowing (`for _, r := range xs`) is not detected; check local bindings if false positives show up.
    let Some(parameters) = std::iter::successors(function_node.parent(), |n| n.parent())
        .find(|n| {
            matches!(
                n.kind(),
                "function_declaration" | "method_declaration" | "func_literal"
            )
        })
        .filter(|function| function.kind() != "func_literal")
        .and_then(|function| function.child_by_field_name("parameters"))
    else {
        return false;
    };
    parameters
        .named_children(&mut parameters.walk())
        .filter(|declaration| {
            declaration
                .children_by_field_name("name", &mut declaration.walk())
                .any(|name| node_text(name, code) == receiver)
        })
        .filter_map(|declaration| declaration.child_by_field_name("type"))
        .any(|kind| {
            let Some((alias, name)) = node_text(kind, code)
                .trim_start_matches('*')
                .split_once('.')
            else {
                return false;
            };
            params.imports.iter().any(|import| {
                imports_package(import, alias)
                    && types.iter().any(|(package, allowed)| {
                        import.orig_module.starts_with(package) && allowed.contains(&name)
                    })
            })
        })
}

/// `alias` names `import`: its alias, or the segment before a `/vN` suffix (`chi/v5` is `chi`).
fn imports_package(import: &Import, alias: &str) -> bool {
    let mut segments = import.orig_module.rsplit('/');
    let versioned = segments
        .next()
        .and_then(|last| last.strip_prefix('v'))
        .is_some_and(|major| !major.is_empty() && major.chars().all(|c| c.is_ascii_digit()));
    import.module_alias == alias || (versioned && segments.next() == Some(alias))
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

fn is_router_origin<'a>(
    origin: &'a str,
    assignments: &'a HashMap<AssignmentKey, Assignment>,
    imports: &[Import],
    seen: &mut Vec<&'a str>,
) -> bool {
    let origin = origin.trim();
    if ROUTER_CONSTRUCTORS
        .iter()
        .any(|constructor| origin.starts_with(constructor))
        || imported_origin_matches(origin, imports, ROUTER_IMPORT_PREFIXES)
    {
        return true;
    }
    if seen.contains(&origin) {
        return false;
    }
    seen.push(origin);
    let Some(base) = [".Group(", ".PathPrefix("]
        .iter()
        .find_map(|method| origin.split_once(method))
        .map(|(base, _)| base.trim())
    else {
        return false;
    };
    let base_origin = assignments
        .values()
        .find(|assignment| assignment.variable_name == base)
        .map_or(base, |assignment| assignment.value.as_str());
    is_router_origin(base_origin, assignments, imports, seen)
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
        // Empty value: bare `var x T` declaration, not a competing origin.
        .filter(|assignment| assignment.variable_name == receiver && !assignment.value.is_empty())
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

pub(super) fn call_arguments(node: Node<'_>) -> Vec<Node<'_>> {
    node.child_by_field_name("arguments")
        .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
        .unwrap_or_default()
}

pub(super) fn normalized_handler(node: Node, code: &str) -> String {
    normalize_whitespace(node_text(node, code))
}

/// Router receiver text, or `None` for a receiverless or package-level call (`http.HandleFunc`).
// ponytail: a local variable shadowing an import alias gets None; check local bindings if that shows up.
pub(super) fn router_variable(function_node: Node, params: &ExtractParams<'_>) -> Option<String> {
    let receiver = function_node.child_by_field_name("operand")?;
    let text = normalize_whitespace(node_text(receiver, params.code));
    let leading = text.split('.').next();
    if params
        .imports
        .iter()
        .any(|import| leading == Some(import.module_alias.as_str()))
    {
        return None;
    }
    Some(text)
}

pub(super) fn function_node(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("function")
}
