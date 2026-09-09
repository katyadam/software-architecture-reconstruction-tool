//! Identification of Java gRPC client calls and server implementations.
//!
//! gRPC generated Java APIs encode the protobuf service in the stub/implementation
//! type (for example `FlightServiceGrpc.FlightServiceBlockingStub`).  We use that
//! stable convention to create a shared operation identifier:
//! `grpc://FlightService/GetById`.  The normal SDG builder can then match a client
//! call to a generated `*ImplBase` implementation without guessing from HTTP
//! URLs.

use std::collections::{HashMap, HashSet};

use models::{CallStatement, Endpoint, HttpMethod, RestCall};
use regex::Regex;
use tree_sitter::Tree;

use crate::extraction::enclosing_lookup::get_hashed_node_value;

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

fn extract_client_calls(code: &str, file_name: &str, calls: &[CallStatement]) -> Vec<RestCall> {
    let stub_fields = Regex::new(
        r"(?m)\b(?P<service>[A-Za-z_][A-Za-z0-9_]*)Grpc\.[A-Za-z_][A-Za-z0-9_]*Stub\s+(?P<field>[A-Za-z_][A-Za-z0-9_]*)",
    ).expect("valid gRPC stub regex");
    let receiver_call =
        Regex::new(r"(?P<receiver>[A-Za-z_][A-Za-z0-9_]*)\.(?P<operation>[a-z][A-Za-z0-9_]*)\s*\(")
            .expect("valid gRPC call regex");

    let stubs: HashMap<String, String> = stub_fields
        .captures_iter(code)
        .map(|capture| (capture["field"].to_string(), capture["service"].to_string()))
        .chain(
            Regex::new(
                r"(?m)\b(?P<field>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*(?P<service>[A-Za-z_][A-Za-z0-9_]*)Grpc\.new(?:Blocking|Future|BlockingV2)?Stub\s*\(",
            )
            .expect("valid gRPC stub initialization regex")
            .captures_iter(code)
            .map(|capture| (capture["field"].to_string(), capture["service"].to_string())),
        )
        .collect();

    let field_calls = receiver_call
        .captures_iter(code)
        .filter_map(|capture| {
            let service = stubs.get(&capture["receiver"])?;
            let operation = upper_first(&capture["operation"]);
            let call_prefix = format!("{}.{}(", &capture["receiver"], &capture["operation"]);
            let call = calls
                .iter()
                .find(|call| call.function_name.starts_with(&call_prefix));

            Some(RestCall {
                function_name: call
                    .and_then(|call| call.enclosing_function_name.clone())
                    .unwrap_or_default(),
                function_hash: call
                    .and_then(|call| call.enclosing_function_hash.clone())
                    .unwrap_or_default(),
                call_arguments: call.map(|call| call.arguments.clone()).unwrap_or_default(),
                // This is an RPC operation, not an HTTP request. POST is used only
                // by the legacy request-shaped SDG model; the URI carries gRPC's
                // actual transport and service identity.
                http_method: HttpMethod::POST,
                target_uri: grpc_uri(service, &operation),
                file_path: file_name.to_string(),
            })
        })
        .collect::<Vec<_>>();

    let direct_stub_call = Regex::new(
        r"\b(?P<service>[A-Za-z_][A-Za-z0-9_]*)Grpc\.new(?:Blocking|Future|BlockingV2)?Stub\s*\([^)]*\)\.(?P<operation>[a-z][A-Za-z0-9_]*)\s*\(",
    )
    .expect("valid direct gRPC stub call regex");
    let direct_calls = direct_stub_call.captures_iter(code).map(|capture| {
        let service = &capture["service"];
        let operation = &capture["operation"];
        let call = calls
            .iter()
            .find(|call| call.function_name.ends_with(&format!(".{operation}(")));
        RestCall {
            function_name: call
                .and_then(|call| call.enclosing_function_name.clone())
                .unwrap_or_default(),
            function_hash: call
                .and_then(|call| call.enclosing_function_hash.clone())
                .unwrap_or_default(),
            call_arguments: call.map(|call| call.arguments.clone()).unwrap_or_default(),
            http_method: HttpMethod::POST,
            target_uri: grpc_uri(service, &upper_first(operation)),
            file_path: file_name.to_string(),
        }
    });

    field_calls.into_iter().chain(direct_calls).collect()
}

fn extract_server_endpoints(code: &str, tree: &Tree, file_name: &str) -> Vec<Endpoint> {
    let service_re = Regex::new(
        r"extends\s+(?P<service>[A-Za-z_][A-Za-z0-9_]*)Grpc\.[A-Za-z_][A-Za-z0-9_]*ImplBase",
    )
    .expect("valid gRPC implementation regex");
    let method_re = Regex::new(
        r"(?m)\b(?:public|private|protected)\s+(?:static\s+)?void\s+(?P<operation>[a-z][A-Za-z0-9_]*)\s*\(",
    )
    .expect("valid gRPC method regex");
    let standard_service = service_re
        .captures(code)
        .map(|capture| capture["service"].to_string());
    let manual_service = Regex::new(
        r"(?s)\bimplements\s+[^\{]*\bBindableService\b.*?(?P<service>[A-Za-z_][A-Za-z0-9_]*)Grpc\.getServiceDescriptor\s*\(\)",
    )
    .expect("valid manual gRPC service regex")
    .captures(code)
    .map(|capture| capture["service"].to_string());
    let Some(service) = standard_service.or(manual_service) else {
        return vec![];
    };

    let manual_operations: HashSet<String> = Regex::new(r"\bMETHOD_(?P<operation>[A-Z][A-Z0-9_]*)")
        .expect("valid manual gRPC method descriptor regex")
        .captures_iter(code)
        .map(|capture| lower_camel_from_constant(&capture["operation"]))
        .collect();
    let is_manual_service = !manual_operations.is_empty();

    method_re
        .captures_iter(code)
        .filter(|capture| !is_manual_service || manual_operations.contains(&capture["operation"]))
        .filter_map(|capture| {
            let operation = upper_first(&capture["operation"]);
            let method_start = capture.get(0)?.start();
            let node = tree
                .root_node()
                .descendant_for_byte_range(method_start, method_start)?;
            let method = find_ancestor_method(node)?;
            Some(Endpoint {
                function_name: method
                    .utf8_text(code.as_bytes())
                    .ok()?
                    .split('{')
                    .next()?
                    .trim()
                    .to_string(),
                function_hash: get_hashed_node_value(method, code),
                http_method: HttpMethod::POST,
                parameters: vec![],
                uri: grpc_uri(&service, &operation),
                file_path: file_name.to_string(),
                router_variable: None,
            })
        })
        .collect()
}

fn lower_camel_from_constant(value: &str) -> String {
    let mut words = value.split('_');
    let Some(first) = words.next() else {
        return String::new();
    };
    first.to_ascii_lowercase()
        + &words
            .map(|word| upper_first(&word.to_ascii_lowercase()))
            .collect::<String>()
}

fn find_ancestor_method(mut node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    loop {
        if node.kind() == "method_declaration" {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn grpc_uri(service: &str, operation: &str) -> String {
    format!("grpc://{service}/{operation}")
}

fn upper_first(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
