use std::collections::HashMap;

use models::{
    CallStatement, CommunicationProtocol, MessageDestinationKind, MessageEdge, MessageRole,
    ParsedCallable, ir::project::TypedFileRecord,
};

use super::shared::package_path;

/// Replaces Go message edges with concrete variants derived from project call sites.
pub(super) fn resolve_message_edges(files: &mut [TypedFileRecord]) {
    let snapshots = files
        .iter()
        .filter(|file| file.language == models::ir::language::Language::Go)
        .map(FileSnapshot::from)
        .collect::<Vec<_>>();

    for file in files
        .iter_mut()
        .filter(|file| file.language == models::ir::language::Language::Go)
    {
        let callables = file.callables.clone();
        file.raw_message_edges = file
            .raw_message_edges
            .iter()
            .flat_map(|edge| {
                let Some(callable) = callables
                    .iter()
                    .find(|callable| callable.metadata.hash == edge.function_hash)
                else {
                    return vec![edge.clone()];
                };
                let resolved = matching_calls(callable, file, &snapshots)
                    .into_iter()
                    .flat_map(|call| resolve_edge(edge, callable, call))
                    .collect::<Vec<_>>();
                if resolved.is_empty() {
                    vec![edge.clone()]
                } else {
                    resolved
                }
            })
            .collect();
    }
}

struct FileSnapshot {
    file_path: String,
    import_modules: Vec<String>,
    calls: Vec<CallStatement>,
}

impl From<&TypedFileRecord> for FileSnapshot {
    /// Retains only the call-site metadata needed for project message resolution.
    fn from(file: &TypedFileRecord) -> Self {
        Self {
            file_path: file.file_path.clone(),
            import_modules: file
                .imports
                .iter()
                .map(|import| import.orig_module.clone())
                .collect(),
            calls: file.call_statements.clone(),
        }
    }
}

/// Finds calls to a callable from its own package or an importing Go package.
fn matching_calls<'a>(
    callable: &'a ParsedCallable,
    target_file: &TypedFileRecord,
    files: &'a [FileSnapshot],
) -> Vec<&'a CallStatement> {
    files
        .iter()
        .filter(|file| package_matches(file, target_file))
        .flat_map(|file| file.calls.iter())
        .filter(|call| {
            call.function_name.rsplit('.').next() == Some(callable.metadata.name.as_str())
                && call.arguments.len() == callable.metadata.parameters.len()
        })
        .collect()
}

/// Checks whether a caller belongs to or imports the callable's package.
fn package_matches(caller: &FileSnapshot, target_file: &TypedFileRecord) -> bool {
    let package = package_path(&target_file.file_path);
    if package_path(&caller.file_path) == package {
        return true;
    }
    let suffix = package
        .rsplit('/')
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("/");
    caller
        .import_modules
        .iter()
        .any(|module| module.replace('\\', "/").ends_with(&suffix))
}

/// Resolves all parameter-backed transport fields for one concrete invocation.
fn resolve_edge(
    edge: &MessageEdge,
    callable: &ParsedCallable,
    call: &CallStatement,
) -> Vec<MessageEdge> {
    let bindings = callable
        .metadata
        .parameters
        .iter()
        .zip(&call.arguments)
        .map(|(parameter, argument)| (parameter.name.as_str(), argument.value.as_str()))
        .collect::<HashMap<_, _>>();
    let exchanges = values_for(edge.exchange.as_deref(), &bindings);
    let routing_keys = values_for(edge.routing_key.as_deref(), &bindings);
    let queues = values_for(edge.queue.as_deref(), &bindings);
    let topics = values_for(edge.topic.as_deref(), &bindings);

    cartesian_edges(edge, exchanges, routing_keys, queues, topics)
}

/// Resolves a field through parameter bindings and expands Go string lists.
fn values_for(value: Option<&str>, bindings: &HashMap<&str, &str>) -> Vec<Option<String>> {
    let Some(value) = value else {
        return vec![None];
    };
    let resolved = bindings.get(value).copied().unwrap_or(value);
    let literals = string_literals(resolved);
    if literals.is_empty() {
        vec![Some(resolved.to_string())]
    } else {
        literals.into_iter().map(Some).collect()
    }
}

/// Produces one edge for every concrete combination of resolved transport fields.
fn cartesian_edges(
    edge: &MessageEdge,
    exchanges: Vec<Option<String>>,
    routing_keys: Vec<Option<String>>,
    queues: Vec<Option<String>>,
    topics: Vec<Option<String>>,
) -> Vec<MessageEdge> {
    let mut edges = Vec::new();
    for exchange in &exchanges {
        for routing_key in &routing_keys {
            for queue in &queues {
                for topic in &topics {
                    let destination = destination(edge, exchange, routing_key, queue, topic);
                    edges.push(MessageEdge {
                        destination,
                        exchange: exchange.clone(),
                        routing_key: routing_key.clone(),
                        queue: queue.clone(),
                        topic: topic.clone(),
                        ..edge.clone()
                    });
                }
            }
        }
    }
    edges
}

/// Rebuilds the destination from resolved RabbitMQ or Kafka transport fields.
fn destination(
    edge: &MessageEdge,
    exchange: &Option<String>,
    routing_key: &Option<String>,
    queue: &Option<String>,
    topic: &Option<String>,
) -> String {
    if edge.protocol == CommunicationProtocol::Kafka
        || matches!(edge.destination_kind, MessageDestinationKind::Topic)
    {
        return topic.clone().unwrap_or_else(|| edge.destination.clone());
    }
    match edge.role {
        MessageRole::Producer | MessageRole::Binding => match (exchange, routing_key) {
            (Some(exchange), Some(routing_key)) if !exchange.is_empty() => {
                format!("{exchange}:{routing_key}")
            }
            (_, Some(routing_key)) => routing_key.clone(),
            (Some(exchange), _) => exchange.clone(),
            _ => queue.clone().unwrap_or_else(|| edge.destination.clone()),
        },
        MessageRole::Consumer | MessageRole::QueueDeclaration | MessageRole::TopicDeclaration => {
            queue.clone().unwrap_or_else(|| edge.destination.clone())
        }
    }
}

/// Extracts interpreted and raw string literals from a Go expression.
fn string_literals(raw: &str) -> Vec<String> {
    let mut values = Vec::new();
    let bytes = raw.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let delimiter = bytes[index];
        if delimiter != b'"' && delimiter != b'`' {
            index += 1;
            continue;
        }
        let start = index + 1;
        index += 1;
        while index < bytes.len() {
            if delimiter == b'"' && bytes[index] == b'\\' {
                index += 2;
                continue;
            }
            if bytes[index] == delimiter {
                values.push(raw[start..index].to_string());
                index += 1;
                break;
            }
            index += 1;
        }
    }
    values
}
