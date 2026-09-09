use std::collections::HashMap;

use models::{
    CallStatement, CommunicationProtocol, MessageDestinationKind, MessageEdge, MessageRole,
};

/// Identifies Go gRPC calls without parsing generated `.pb.go` files. Go applications expose
/// the required protocol information at their generated client construction and server
/// registration call sites, which are regular source files and therefore safe to analyze.
pub(super) fn identify_message_edges(
    call: &CallStatement,
    file_path: &str,
    scope: &HashMap<String, String>,
    is_grpc_file: bool,
) -> Vec<MessageEdge> {
    let Some(last) = call.function_name.rsplit('.').next() else {
        return Vec::new();
    };

    if let Some(service) = last
        .strip_prefix("Register")
        .and_then(|name| name.strip_suffix("Server"))
        .filter(|name| !name.is_empty())
    {
        return vec![edge(
            MessageRole::Consumer,
            service.to_string(),
            call,
            file_path,
        )];
    }

    // Construction alone is not an RPC. The variable binding is consumed when its method is
    // invoked below; retaining this branch makes the deliberate non-match explicit.
    if last.starts_with("New") && last.ends_with("Client") {
        return Vec::new();
    }

    let Some((receiver, method)) = call.function_name.rsplit_once('.') else {
        return Vec::new();
    };
    // RPC invocation often lives in a handler package that imports only the generated `pb`
    // package; the `grpc` import is isolated in a connection helper. In that layout, a local
    // `New<Service>Client` binding or a `<service>Client` field is sufficient provenance.
    if !is_grpc_file
        && !scope
            .values()
            .any(|value| client_service_name(value).is_some())
        && receiver
            .rsplit('.')
            .next()
            .is_none_or(|name| !name.ends_with("Client") || name.eq_ignore_ascii_case("client"))
    {
        return Vec::new();
    }
    service_from_receiver(receiver, scope)
        .map(|service| {
            edge(
                MessageRole::Producer,
                format!("{service}/{method}"),
                call,
                file_path,
            )
        })
        .into_iter()
        .collect()
}

fn service_from_receiver(receiver: &str, scope: &HashMap<String, String>) -> Option<String> {
    let binding = scope.get(receiver).map(String::as_str).unwrap_or(receiver);
    if let Some(service) = client_service_name(binding) {
        return Some(service);
    }
    // Keep receiver fields intact for message_edge_project. It can follow the field through
    // the receiver type's `New...` constructor across files, which is more reliable than a
    // field-name guess (for example `s.geoClient`).
    if receiver.starts_with("s.") {
        return Some(receiver.to_string());
    }
    // `u.usersClient.CreateUser` is common when a generated client is stored on a struct.
    // Its type is unavailable in the lightweight call IR, but the field name preserves the
    // service convention used by generated Go clients.
    receiver
        .rsplit('.')
        .next()
        .and_then(|field| field.strip_suffix("Client"))
        .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("client"))
        .map(|name| format!("{}Service", upper_first(name)))
}

pub(super) fn client_service_name(value: &str) -> Option<String> {
    let marker = "New";
    let start = value.rfind(marker)? + marker.len();
    let remainder = &value[start..];
    let end = remainder.find("Client(")?;
    let service = &remainder[..end];
    (!service.is_empty()).then(|| service.to_string())
}

fn upper_first(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_uppercase().collect::<String>() + chars.as_str()
}

fn edge(
    role: MessageRole,
    destination: String,
    call: &CallStatement,
    file_path: &str,
) -> MessageEdge {
    MessageEdge {
        protocol: CommunicationProtocol::Grpc,
        role,
        destination_kind: MessageDestinationKind::GrpcService,
        destination,
        exchange: None,
        routing_key: None,
        queue: None,
        topic: None,
        handler: None,
        function_name: call.enclosing_function_name.clone().unwrap_or_default(),
        function_hash: call.enclosing_function_hash.clone().unwrap_or_default(),
        call_arguments: call.arguments.clone(),
        file_path: file_path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{client_service_name, service_from_receiver};

    #[test]
    fn resolves_generated_client_bindings_and_struct_fields() {
        assert_eq!(
            client_service_name("pb.NewProductServiceClient(conn)"),
            Some("ProductService".to_string())
        );
        assert_eq!(
            service_from_receiver(
                "client",
                &HashMap::from([(
                    "client".to_string(),
                    "pb.NewProductServiceClient(conn)".to_string()
                )])
            ),
            Some("ProductService".to_string())
        );
        assert_eq!(
            service_from_receiver("u.usersClient", &HashMap::new()),
            Some("UsersService".to_string())
        );
    }
}
