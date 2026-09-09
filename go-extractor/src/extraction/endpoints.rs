use std::collections::{HashMap, HashSet};

use models::{
    Assignment, AssignmentKey, Callable, Endpoint, HttpMethod, Import, Namespace, ParsedCallable,
};
use statix::strings::hash_text;
use tree_sitter::Node;

use super::endpoint_frameworks::{ExtractParams, strategies};
use super::shared::{
    SYNTHETIC_HANDLER_PREFIX, format_http_method, lookup_callable, scope_bindings, walk_named,
};

pub(super) fn collect_endpoints(
    root: Node,
    code: &str,
    file_path: &str,
    assignments: &HashMap<AssignmentKey, Assignment>,
    imports: &[Import],
    callable_lookup: &HashMap<String, Callable>,
    synthetic_callables: &mut Vec<ParsedCallable>,
) -> Vec<Endpoint> {
    let globals = scope_bindings(assignments, &models::Scope::Global);
    let mut synthetic_hashes = HashSet::new();
    let mut endpoints = Vec::new();

    walk_named(root, &mut |node| {
        if node.kind() != "call_expression" {
            return;
        }

        if let Some(endpoint) = endpoint_from_call(
            node,
            code,
            file_path,
            &globals,
            assignments,
            imports,
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
    assignments: &HashMap<AssignmentKey, Assignment>,
    imports: &[Import],
    callable_lookup: &HashMap<String, Callable>,
    synthetic_callables: &mut Vec<ParsedCallable>,
    synthetic_hashes: &mut HashSet<String>,
) -> Option<Endpoint> {
    let params = ExtractParams {
        node,
        code,
        globals,
        assignments,
        imports,
    };
    for strategy in strategies() {
        if let Some(endpoint) = strategy.identify(&params) {
            return Some(build_endpoint(
                file_path,
                &endpoint.path,
                endpoint.method,
                &endpoint.handler,
                callable_lookup,
                synthetic_callables,
                synthetic_hashes,
            ));
        }
    }

    None
}

fn build_endpoint(
    file_path: &str,
    uri: &str,
    method: HttpMethod,
    handler_expr: &str,
    callable_lookup: &HashMap<String, Callable>,
    synthetic_callables: &mut Vec<ParsedCallable>,
    synthetic_hashes: &mut HashSet<String>,
) -> Endpoint {
    let callable = resolve_handler_callable(
        file_path,
        handler_expr,
        uri,
        &method,
        callable_lookup,
        synthetic_callables,
        synthetic_hashes,
    );
    Endpoint {
        function_name: callable.signature.clone(),
        function_hash: callable.hash,
        http_method: method,
        parameters: vec![],
        uri: uri.to_string(),
        file_path: file_path.to_string(),
        router_variable: None,
    }
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

    let signature = format!(
        "{SYNTHETIC_HANDLER_PREFIX}{} {}",
        format_http_method(method),
        uri
    );
    let hash = hash_text(&format!("{file_path}::{signature}::{handler_expr}"));
    if synthetic_hashes.insert(hash.clone()) {
        synthetic_callables.push(ParsedCallable {
            metadata: Callable {
                name: handler_expr.to_string(),
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
