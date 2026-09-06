use std::collections::HashMap;

use models::{
    CallStatement, CommunicationProtocol, MessageDestinationKind, MessageEdge, MessageRole,
};

const DESTINATION_SEPARATOR: &str = ":";

/// Dispatches a Go call to the corresponding RabbitMQ edge recognizer.
pub(super) fn identify_message_edge(
    call: &CallStatement,
    file_path: &str,
    scope: &HashMap<String, String>,
) -> Option<MessageEdge> {
    let method = call.function_name.split('.').next_back()?;
    match method {
        "PublishWithContext" => identify_publish(call, file_path, scope),
        "Publish" => identify_publish_without_context(call, file_path, scope),
        "QueueBind" => identify_binding(call, file_path, scope),
        "Consume" => identify_consume(call, file_path, scope),
        "QueueDeclare" => identify_queue_declaration(call, file_path, scope),
        _ => None,
    }
}

/// Identifies `Channel.Publish` calls that do not take a context argument.
fn identify_publish_without_context(
    call: &CallStatement,
    file_path: &str,
    scope: &HashMap<String, String>,
) -> Option<MessageEdge> {
    let exchange = clean_arg(call.arguments.first()?.value.as_str(), scope);
    let routing_key = clean_arg(call.arguments.get(1)?.value.as_str(), scope);
    build_publish_edge(call, file_path, exchange, routing_key)
}

/// Identifies `Channel.PublishWithContext` calls.
fn identify_publish(
    call: &CallStatement,
    file_path: &str,
    scope: &HashMap<String, String>,
) -> Option<MessageEdge> {
    let exchange = clean_arg(call.arguments.get(1)?.value.as_str(), scope);
    let routing_key = clean_arg(call.arguments.get(2)?.value.as_str(), scope);
    build_publish_edge(call, file_path, exchange, routing_key)
}

/// Builds a producer edge using the resolved exchange and routing key.
fn build_publish_edge(
    call: &CallStatement,
    file_path: &str,
    exchange: String,
    routing_key: String,
) -> Option<MessageEdge> {
    if exchange.is_empty() && routing_key.is_empty() {
        return None;
    }

    let destination_kind = if exchange.is_empty() {
        MessageDestinationKind::Queue
    } else {
        MessageDestinationKind::ExchangeRoutingKey
    };
    let destination = if exchange.is_empty() {
        routing_key.clone()
    } else if routing_key.is_empty() {
        exchange.clone()
    } else {
        format!("{exchange}{DESTINATION_SEPARATOR}{routing_key}")
    };

    Some(edge(
        call,
        file_path,
        MessageRole::Producer,
        destination_kind,
        destination,
        non_empty(exchange),
        non_empty(routing_key.clone()),
        non_empty(routing_key),
    ))
}

/// Identifies `Channel.QueueBind` declarations as exchange-routing-key bindings.
fn identify_binding(
    call: &CallStatement,
    file_path: &str,
    scope: &HashMap<String, String>,
) -> Option<MessageEdge> {
    let queue = clean_arg(call.arguments.first()?.value.as_str(), scope);
    let routing_key = clean_arg(call.arguments.get(1)?.value.as_str(), scope);
    let exchange = clean_arg(call.arguments.get(2)?.value.as_str(), scope);
    if exchange.is_empty() && routing_key.is_empty() && queue.is_empty() {
        return None;
    }

    let destination_kind = if exchange.is_empty() {
        MessageDestinationKind::Queue
    } else {
        MessageDestinationKind::ExchangeRoutingKey
    };
    let destination = if exchange.is_empty() {
        queue.clone()
    } else if routing_key.is_empty() {
        exchange.clone()
    } else {
        format!("{exchange}{DESTINATION_SEPARATOR}{routing_key}")
    };

    Some(edge(
        call,
        file_path,
        MessageRole::Binding,
        destination_kind,
        destination,
        non_empty(exchange),
        non_empty(routing_key),
        non_empty(queue),
    ))
}

/// Identifies `Channel.Consume` calls as queue consumers.
fn identify_consume(
    call: &CallStatement,
    file_path: &str,
    scope: &HashMap<String, String>,
) -> Option<MessageEdge> {
    let queue = clean_arg(call.arguments.first()?.value.as_str(), scope);
    if queue.is_empty() {
        return None;
    }

    Some(edge(
        call,
        file_path,
        MessageRole::Consumer,
        MessageDestinationKind::Queue,
        queue.clone(),
        None,
        None,
        Some(queue),
    ))
}

/// Identifies `Channel.QueueDeclare` calls as queue declarations.
fn identify_queue_declaration(
    call: &CallStatement,
    file_path: &str,
    scope: &HashMap<String, String>,
) -> Option<MessageEdge> {
    let queue = clean_arg(call.arguments.first()?.value.as_str(), scope);
    if queue.is_empty() {
        return None;
    }
    Some(edge(
        call,
        file_path,
        MessageRole::QueueDeclaration,
        MessageDestinationKind::Queue,
        queue.clone(),
        None,
        None,
        Some(queue),
    ))
}

/// Builds a RabbitMQ message edge with the originating call metadata.
fn edge(
    call: &CallStatement,
    file_path: &str,
    role: MessageRole,
    destination_kind: MessageDestinationKind,
    destination: String,
    exchange: Option<String>,
    routing_key: Option<String>,
    queue: Option<String>,
) -> MessageEdge {
    MessageEdge {
        protocol: CommunicationProtocol::RabbitMq,
        role,
        destination_kind,
        destination,
        exchange,
        routing_key,
        queue,
        topic: None,
        handler: None,
        function_name: call.enclosing_function_name.clone().unwrap_or_default(),
        function_hash: call.enclosing_function_hash.clone().unwrap_or_default(),
        call_arguments: call.arguments.clone(),
        file_path: file_path.to_string(),
    }
}

/// Resolves a Go expression and removes surrounding string syntax.
fn clean_arg(raw: &str, scope: &HashMap<String, String>) -> String {
    let resolved = super::shared::evaluate_expression_text(raw, scope);
    let trimmed = resolved.trim().trim_end_matches(',');
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('`') && trimmed.ends_with('`'))
    {
        return trimmed[1..trimmed.len() - 1].to_string();
    }
    trimmed.to_string()
}

/// Converts an empty transport field into `None`.
fn non_empty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}
