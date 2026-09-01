//! Identification of Python gRPC aio stubs and servicers.

use std::collections::HashSet;

use models::{CallStatement, Endpoint, HttpMethod, RestCall};
use regex::Regex;
use tree_sitter::Tree;

pub fn extract(
    code: &str,
    tree: &Tree,
    file_name: &str,
    calls: &[CallStatement],
) -> (Vec<Endpoint>, Vec<RestCall>) {
    (
        extract_server_endpoints(code, tree, file_name),
        extract_client_calls(code, file_name, calls),
    )
}

#[allow(clippy::unnecessary_filter_map)]
fn extract_client_calls(code: &str, file_name: &str, calls: &[CallStatement]) -> Vec<RestCall> {
    let stub_re =
        Regex::new(r"\b[A-Za-z_][A-Za-z0-9_]*\.(?P<service>[A-Za-z_][A-Za-z0-9_]*)Stub\s*\(")
            .expect("valid gRPC Python stub regex");
    let operation_re = Regex::new(r"\bself\.stub\.(?P<operation>[A-Z][A-Za-z0-9_]*)\s*\(")
        .expect("valid gRPC Python operation regex");
    let services: HashSet<String> = stub_re
        .captures_iter(code)
        .map(|capture| capture["service"].to_string())
        .collect();
    if services.len() != 1 {
        return vec![];
    }
    let service = services.into_iter().next().expect("one service");

    operation_re
        .captures_iter(code)
        .filter_map(|capture| {
            let operation = &capture["operation"];
            let prefix = format!("self.stub.{operation}(");
            let call = calls.iter().find(|call| {
                call.function_name.starts_with(&prefix) || call.function_name.ends_with(operation)
            });
            Some(RestCall {
                function_name: call
                    .and_then(|call| call.enclosing_function_name.clone())
                    .unwrap_or_default(),
                function_hash: call
                    .and_then(|call| call.enclosing_function_hash.clone())
                    .unwrap_or_default(),
                call_arguments: call.map(|call| call.arguments.clone()).unwrap_or_default(),
                http_method: HttpMethod::POST,
                target_uri: grpc_uri(&service, operation),
                file_path: file_name.to_string(),
            })
        })
        .collect()
}

fn extract_server_endpoints(code: &str, tree: &Tree, file_name: &str) -> Vec<Endpoint> {
    let service_re = Regex::new(
        r"class\s+[A-Za-z_][A-Za-z0-9_]*\s*\([^\n)]*\.(?P<service>[A-Za-z_][A-Za-z0-9_]*)Servicer\)",
    ).expect("valid gRPC Python servicer regex");
    let method_re = Regex::new(r"(?m)^\s*async\s+def\s+(?P<operation>[A-Z][A-Za-z0-9_]*)\s*\(")
        .expect("valid gRPC Python method regex");
    let Some(service) = service_re
        .captures(code)
        .map(|capture| capture["service"].to_string())
    else {
        return vec![];
    };

    method_re
        .captures_iter(code)
        .filter_map(|capture| {
            let method = find_function_by_name(tree.root_node(), &capture["operation"], code)?;
            Some(Endpoint {
                function_name: method
                    .utf8_text(code.as_bytes())
                    .ok()?
                    .lines()
                    .next()?
                    .trim()
                    .to_string(),
                function_hash: statix::strings::hash_text(method.utf8_text(code.as_bytes()).ok()?),
                http_method: HttpMethod::POST,
                parameters: vec![],
                uri: grpc_uri(&service, &capture["operation"]),
                file_path: file_name.to_string(),
                router_variable: None,
            })
        })
        .collect()
}

fn find_function_by_name<'a>(
    node: tree_sitter::Node<'a>,
    name: &str,
    code: &str,
) -> Option<tree_sitter::Node<'a>> {
    if node.kind() == "function_definition"
        && node
            .child_by_field_name("name")?
            .utf8_text(code.as_bytes())
            .ok()?
            == name
    {
        return Some(node);
    }
    (0..node.child_count())
        .find_map(|index| find_function_by_name(node.child(index as u32)?, name, code))
}

fn grpc_uri(service: &str, operation: &str) -> String {
    format!("grpc://{service}/{operation}")
}
