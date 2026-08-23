use std::collections::{HashMap, HashSet};

use models::{
    Assignment, AssignmentKey, Callable, Endpoint, HttpMethod, Namespace, ParsedCallable,
};
use statix::strings::{hash_text, normalize_whitespace};
use tree_sitter::Node;

use super::shared::{
    GoNodeKind, evaluate_expression_node, format_http_method, go_node_kind, lookup_callable,
    node_text, parse_http_method_value, scope_bindings, selector_name, split_method_and_path,
    walk_named, web_route_method,
};

pub(super) fn collect_endpoints(
    root: Node,
    code: &str,
    file_path: &str,
    assignments: &HashMap<AssignmentKey, Assignment>,
    callable_lookup: &HashMap<String, Callable>,
    synthetic_callables: &mut Vec<ParsedCallable>,
) -> Vec<Endpoint> {
    let globals = scope_bindings(assignments, &models::Scope::Global);
    let mut synthetic_hashes = HashSet::new();
    let mut endpoints = Vec::new();

    walk_named(root, &mut |node| {
        if go_node_kind(node) != GoNodeKind::CallExpression {
            return;
        }

        if let Some(endpoint) = endpoint_from_call(
            node,
            code,
            file_path,
            &globals,
            callable_lookup,
            synthetic_callables,
            &mut synthetic_hashes,
        ) {
            endpoints.push(endpoint);
        }
    });

    endpoints
}

fn endpoint_from_call(
    node: Node,
    code: &str,
    file_path: &str,
    globals: &HashMap<String, String>,
    callable_lookup: &HashMap<String, Callable>,
    synthetic_callables: &mut Vec<ParsedCallable>,
    synthetic_hashes: &mut HashSet<String>,
) -> Option<Endpoint> {
    let function_node = node.child_by_field_name("function")?;

    if let Some((method, path, handler)) = gorilla_route_parts(node, code, globals) {
        let callable = resolve_handler_callable(
            file_path,
            &handler,
            &path,
            &method,
            callable_lookup,
            synthetic_callables,
            synthetic_hashes,
        );
        return Some(Endpoint {
            function_name: callable.signature.clone(),
            function_hash: callable.hash,
            http_method: method,
            parameters: vec![],
            uri: path,
            file_path: file_path.to_string(),
            router_variable: None,
        });
    }

    let selector = selector_name(function_node, code)?;
    if selector == "HandleFunc" {
        let arguments = node
            .child_by_field_name("arguments")
            .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
            .unwrap_or_default();
        if arguments.len() < 2 {
            return None;
        }
        let resolved = evaluate_expression_node(arguments[0], code, globals);
        let (method, uri) = split_method_and_path(&resolved)?;
        let handler_expr = normalize_whitespace(node_text(arguments[1], code));
        let callable = resolve_handler_callable(
            file_path,
            &handler_expr,
            &uri,
            &method,
            callable_lookup,
            synthetic_callables,
            synthetic_hashes,
        );
        return Some(Endpoint {
            function_name: callable.signature.clone(),
            function_hash: callable.hash,
            http_method: method,
            parameters: vec![],
            uri,
            file_path: file_path.to_string(),
            router_variable: None,
        });
    }

    if let Some(method) = web_route_method(&selector) {
        let arguments = node
            .child_by_field_name("arguments")
            .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
            .unwrap_or_default();
        if arguments.len() < 2 {
            return None;
        }
        let uri = evaluate_expression_node(arguments[0], code, globals);
        let handler_expr = normalize_whitespace(node_text(arguments[1], code));
        let callable = resolve_handler_callable(
            file_path,
            &handler_expr,
            &uri,
            &method,
            callable_lookup,
            synthetic_callables,
            synthetic_hashes,
        );
        return Some(Endpoint {
            function_name: callable.signature.clone(),
            function_hash: callable.hash,
            http_method: method,
            parameters: vec![],
            uri,
            file_path: file_path.to_string(),
            router_variable: None,
        });
    }

    None
}

fn gorilla_route_parts(
    node: Node,
    code: &str,
    globals: &HashMap<String, String>,
) -> Option<(HttpMethod, String, String)> {
    let function_node = node.child_by_field_name("function")?;
    if selector_name(function_node, code)? != "Methods" {
        return None;
    }

    let selector_operand = function_node.child_by_field_name("operand")?;
    if go_node_kind(selector_operand) != GoNodeKind::CallExpression {
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

fn resolve_handler_callable(
    file_path: &str,
    handler_expr: &str,
    uri: &str,
    method: &HttpMethod,
    callable_lookup: &HashMap<String, Callable>,
    synthetic_callables: &mut Vec<ParsedCallable>,
    synthetic_hashes: &mut HashSet<String>,
) -> Callable {
    if let Some(actual) = lookup_callable(handler_expr, callable_lookup) {
        return actual;
    }

    let signature = format!("handler {} {}", format_http_method(method), uri);
    let hash = hash_text(&format!("{file_path}::{signature}::{handler_expr}"));
    if synthetic_hashes.insert(hash.clone()) {
        synthetic_callables.push(ParsedCallable {
            metadata: Callable {
                name: signature.clone(),
                signature: signature.clone(),
                namespace: Namespace::Module(file_path.to_string()),
                parameters: vec![],
                return_type: None,
                is_async: false,
                is_constructor: false,
                hash: hash.clone(),
                file_path: file_path.to_string(),
            },
            ast: models::ir::ast::CallableAst {
                statements: vec![],
                nested: vec![],
            },
        });
    }

    Callable {
        name: signature.clone(),
        signature,
        namespace: Namespace::Module(file_path.to_string()),
        parameters: vec![],
        return_type: None,
        is_async: false,
        is_constructor: false,
        hash,
        file_path: file_path.to_string(),
    }
}
