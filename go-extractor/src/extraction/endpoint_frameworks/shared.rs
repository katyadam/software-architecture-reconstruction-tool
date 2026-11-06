use std::collections::HashMap;

use models::{Assignment, AssignmentKey, HttpMethod, Import, ParsedCallStatement};
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
        .any(|kind| is_router_type(node_text(kind, code), params.imports, types))
}

/// `kind` is `[*]<alias>.<Type>` with `Type` allowed for the package `alias` imports.
fn is_router_type(kind: &str, imports: &[Import], types: &[(&str, &[&str])]) -> bool {
    let Some((alias, name)) = kind.trim_start_matches('*').split_once('.') else {
        return false;
    };
    imports.iter().any(|import| {
        imports_package(import, alias)
            && types.iter().any(|(package, allowed)| {
                import.orig_module.starts_with(package) && allowed.contains(&name)
            })
    })
}

/// Function literal passed to `.Route` / `.Group`, whose single parameter is a router.
pub(crate) struct ClosureRouter<'a> {
    pub call: &'a ParsedCallStatement,
    pub parameter: &'a str,
    pub receiver: &'a str,
    /// Prefix expression text; `""` for `.Group`.
    pub prefix: &'a str,
}

impl ClosureRouter<'_> {
    /// Synthetic variable: an inner and an outer `r` share one resolver scope.
    /// End byte: chained `.Route(..).Route(..)` calls share their start.
    pub(crate) fn variable(&self) -> String {
        format!("{}@{}", self.parameter, self.call.call_span.end_byte)
    }
}

/// Closure routers among `calls`.
pub(crate) fn closure_routers<'a>(
    calls: &'a [ParsedCallStatement],
    imports: &[Import],
) -> Vec<ClosureRouter<'a>> {
    calls
        .iter()
        .filter_map(|call| {
            let (receiver, method) = call.metadata.function_name.rsplit_once('.')?;
            let (prefix, literal) = match (method, call.metadata.arguments.as_slice()) {
                ("Route", [prefix, literal, ..]) => (prefix.value.as_str(), literal),
                ("Group", [literal, ..]) => ("\"\"", literal),
                _ => return None,
            };
            Some(ClosureRouter {
                call,
                parameter: closure_parameter(&literal.value, imports)?,
                receiver: receiver.trim(),
                prefix,
            })
        })
        .collect()
}

/// Name of the router parameter of `func(<name> <type>)`.
fn closure_parameter<'a>(literal: &'a str, imports: &[Import]) -> Option<&'a str> {
    let rest = literal
        .strip_prefix("func")?
        .trim_start()
        .strip_prefix('(')?;
    let (parameters, _) = rest.split_once(')')?;
    // gofmt keeps a trailing comma on a multi-line parameter list.
    let parameters = parameters.trim().trim_end_matches(',');
    let [name, kind] = parameters.split_whitespace().collect::<Vec<_>>()[..] else {
        return None;
    };
    is_router_type(kind, imports, ROUTER_PARAMETER_TYPES).then_some(name)
}

/// `name` at `position`: innermost enclosing closure router with that parameter, else `name`.
/// Strict start: a call's own receiver chain shares its start byte and is not in its closure.
/// So is an earlier closure of a chain, which ends before the later call does.
// ponytail: full scan per lookup, quadratic in a file's closures; index by start if a real file gets slow.
pub(crate) fn scoped_router(closures: &[ClosureRouter<'_>], position: u32, name: &str) -> String {
    closures
        .iter()
        .filter(|closure| {
            let span = &closure.call.call_span;
            closure.parameter == name
                && span.start_byte < position
                && position < span.end_byte
                && !closures.iter().any(|earlier| {
                    let other = &earlier.call.call_span;
                    other.start_byte == span.start_byte
                        && other.end_byte < span.end_byte
                        && position < other.end_byte
                })
        })
        .min_by_key(|closure| closure.call.call_span.end_byte)
        .map_or_else(|| name.to_string(), ClosureRouter::variable)
}

/// Synthetic variable of the closure router the call is made on, if any.
pub(super) fn closure_router(function_node: Node, params: &ExtractParams<'_>) -> Option<String> {
    let receiver = function_node
        .child_by_field_name("operand")
        .filter(|operand| operand.kind() == "identifier")?;
    let receiver = node_text(receiver, params.code);
    let scoped = scoped_router(params.closures, params.node.start_byte() as u32, receiver);
    (scoped != receiver).then_some(scoped)
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

#[cfg(test)]
mod tests {
    use super::{closure_routers, scoped_router};
    use crate::extraction::extract_syntactic;

    /// `scoped_router` for `name` at the first occurrence of `at` in a chi file's `body`.
    fn scoped(body: &str, at: &str, name: &str) -> String {
        let code = format!(
            "package main\n\nimport \"github.com/go-chi/chi/v5\"\nimport \"github.com/gin-gonic/gin\"\n\nfunc routes() {{\n{body}\n}}\n"
        );
        let record = extract_syntactic(&code, "x.go").expect("Go parses");
        let closures = closure_routers(&record.call_statements, &record.imports);
        let position = code.find(at).expect("marker in code") as u32;
        // Digits dropped: tests assert synthetic or not, not the byte.
        scoped_router(&closures, position, name).replace(|c: char| c.is_ascii_digit(), "")
    }

    #[test]
    fn inside_closure_is_synthetic() {
        let body = r#"r.Route("/p", func(sub chi.Router) { sub.Get("/x", h) })"#;
        assert_eq!(scoped(body, "sub.Get", "sub"), "sub@");
    }

    #[test]
    fn other_name_stays() {
        let body = r#"r.Route("/p", func(sub chi.Router) { r.Get("/x", h) })"#;
        assert_eq!(scoped(body, "r.Get", "r"), "r");
    }

    #[test]
    fn outside_closure_stays() {
        let body = r#"r.Route("/p", func(r chi.Router) {}); r.Get("/x", h)"#;
        assert_eq!(scoped(body, "r.Get", "r"), "r");
    }

    #[test]
    fn receiver_chain_is_outside_its_closure() {
        let body = r#"r.Route("/p", func(r chi.Router) {}).Get("/x", h)"#;
        assert_eq!(scoped(body, "r.Route", "r"), "r");
    }

    #[test]
    fn earlier_closure_of_a_chain_is_outside_the_later_one() {
        let body = r#"r.Route("/a", func(x chi.Router) { r.Get("/z", h) }).Route("/b", func(r chi.Router) {})"#;
        assert_eq!(scoped(body, "r.Get", "r"), "r");
    }

    #[test]
    fn shadowing_picks_innermost() {
        let code = "package main\n\nimport \"github.com/go-chi/chi/v5\"\n\nfunc routes() {\n\tr.Route(\"/a\", func(r chi.Router) { r.Route(\"/b\", func(r chi.Router) { r.Get(\"/x\", h) }) })\n}\n";
        let record = extract_syntactic(code, "x.go").expect("Go parses");
        let closures = closure_routers(&record.call_statements, &record.imports);
        let inner = code.find("})").expect("inner call end") as u32 + 2;
        let get = code.find("r.Get").expect("route") as u32;
        assert_eq!(scoped_router(&closures, get, "r"), format!("r@{inner}"));
    }

    #[test]
    fn nesting_reaches_outer_parameter() {
        let code = "package main\n\nimport \"github.com/go-chi/chi/v5\"\n\nfunc routes() {\n\tr.Route(\"/a\", func(a chi.Router) { a.Route(\"/b\", func(b chi.Router) { a.Get(\"/x\", h) }) })\n}\n";
        let record = extract_syntactic(code, "x.go").expect("Go parses");
        let closures = closure_routers(&record.call_statements, &record.imports);
        let outer = code.rfind("})").expect("outer call end") as u32 + 2;
        let get = code.find("a.Get").expect("route") as u32;
        assert_eq!(scoped_router(&closures, get, "a"), format!("a@{outer}"));
    }

    #[test]
    fn unrecognized_closures_rejected() {
        for literal in [
            "func(r Local) { r.Get(\"/x\", h) }",
            "func(r *gin.Context) { r.Get(\"/x\", h) }",
            "func(a, r chi.Router) { r.Get(\"/x\", h) }",
        ] {
            let body = format!("r.Route(\"/p\", {literal})");
            assert_eq!(scoped(&body, "r.Get", "r"), "r", "{literal}");
        }
    }
}
