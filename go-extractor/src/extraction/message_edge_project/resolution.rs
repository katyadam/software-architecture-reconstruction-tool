use std::collections::{HashMap, HashSet};

use models::{
    CommunicationProtocol, MessageDestinationKind, MessageEdge, MessageRole, ParsedCallable,
};

use super::{FileSnapshot, Invocation, matching_calls, snapshot_for, values::values_for};
use crate::extraction::grpc_message_edges::client_service_name;

const MAX_RESOLUTION_DEPTH: usize = 4;

/// Resolves one invocation and follows its enclosing wrapper when it has callers.
pub(super) fn resolve_invocation(
    edge: &MessageEdge,
    callable: &ParsedCallable,
    invocation: Invocation<'_>,
    files: &[FileSnapshot],
    known_values: &HashMap<String, Vec<String>>,
    depth: usize,
    visited: &mut HashSet<String>,
) -> Vec<MessageEdge> {
    let resolved = resolve_edge(edge, callable, &invocation, files, known_values);
    if depth >= MAX_RESOLUTION_DEPTH {
        return resolved;
    }
    let Some(parent) = invocation.file.callables.iter().find(|candidate| {
        invocation.call.enclosing_function_hash.as_ref() == Some(&candidate.metadata.hash)
            || invocation.call.enclosing_function_name.as_deref()
                == Some(candidate.metadata.name.as_str())
            || invocation.call.enclosing_function_name.as_deref()
                == Some(candidate.metadata.signature.as_str())
    }) else {
        return resolved;
    };
    let visit_key = format!("{}:{}", invocation.file.file_path, parent.metadata.hash);
    if !visited.insert(visit_key) {
        return resolved;
    }
    let parent_calls = matching_calls(parent, invocation.file, files);
    if parent_calls.is_empty() {
        return resolved;
    }
    let propagated = resolved
        .iter()
        .flat_map(|edge| {
            parent_calls.iter().flat_map(|parent_call| {
                let mut route = visited.clone();
                resolve_invocation(
                    edge,
                    parent,
                    Invocation {
                        call: parent_call.call,
                        file: parent_call.file,
                    },
                    files,
                    known_values,
                    depth + 1,
                    &mut route,
                )
            })
        })
        .collect::<Vec<_>>();
    if propagated.is_empty() {
        resolved
    } else {
        propagated
    }
}

/// Resolves all parameter-backed transport fields for one concrete invocation.
fn resolve_edge(
    edge: &MessageEdge,
    callable: &ParsedCallable,
    invocation: &Invocation<'_>,
    files: &[FileSnapshot],
    known_values: &HashMap<String, Vec<String>>,
) -> Vec<MessageEdge> {
    let bindings = callable
        .metadata
        .parameters
        .iter()
        .zip(&invocation.call.arguments)
        .map(|(parameter, argument)| (parameter.name.as_str(), argument.value.as_str()))
        .collect::<HashMap<_, _>>();
    resolve_edge_fields(
        edge,
        callable,
        &bindings,
        &invocation.file.file_path,
        files,
        known_values,
    )
}

/// Resolves constructor-backed transport fields without applying any call arguments.
pub(super) fn resolve_declared_edge(
    edge: &MessageEdge,
    callable: &ParsedCallable,
    source: &FileSnapshot,
    files: &[FileSnapshot],
    known_values: &HashMap<String, Vec<String>>,
) -> Vec<MessageEdge> {
    resolve_edge_fields(
        edge,
        callable,
        &HashMap::new(),
        &source.file_path,
        files,
        known_values,
    )
}

/// Resolves transport fields using the supplied parameter bindings and source file path.
fn resolve_edge_fields(
    edge: &MessageEdge,
    callable: &ParsedCallable,
    bindings: &HashMap<&str, &str>,
    file_path: &str,
    files: &[FileSnapshot],
    known_values: &HashMap<String, Vec<String>>,
) -> Vec<MessageEdge> {
    let aliases = snapshot_for(files, &callable.metadata.file_path)
        .map(|file| file.assignments.iter().cloned().collect::<HashMap<_, _>>())
        .unwrap_or_default();
    let exchanges = values_for(
        edge.exchange.as_deref(),
        &bindings,
        &aliases,
        known_values,
        callable,
        files,
    );
    let routing_keys = values_for(
        edge.routing_key.as_deref(),
        &bindings,
        &aliases,
        known_values,
        callable,
        files,
    );
    let queues = values_for(
        edge.queue.as_deref(),
        &bindings,
        &aliases,
        known_values,
        callable,
        files,
    );
    let topics = values_for(
        edge.topic.as_deref(),
        &bindings,
        &aliases,
        known_values,
        callable,
        files,
    );

    let destinations = grpc_destinations(edge, callable, bindings, &aliases, known_values, files);
    cartesian_edges(edge, exchanges, routing_keys, queues, topics)
        .into_iter()
        .flat_map(|edge| {
            destinations.iter().map(move |destination| MessageEdge {
                destination: destination.clone(),
                ..edge.clone()
            })
        })
        // QueueBind commonly reuses one loop variable for its queue and routing key.
        // Keep those expansions paired instead of constructing a cross-product.
        .filter(|resolved| {
            edge.role != MessageRole::Binding
                || edge.queue != edge.routing_key
                || resolved.queue == resolved.routing_key
        })
        .map(|edge| MessageEdge {
            file_path: file_path.to_string(),
            ..edge
        })
        .collect()
}

/// Resolves an RPC receiver retained by the Go gRPC extractor. For example,
/// `s.geoClient/Nearby` becomes `Geo/Nearby` by following `geoClient` through `New()`.
fn grpc_destinations(
    edge: &MessageEdge,
    callable: &ParsedCallable,
    bindings: &HashMap<&str, &str>,
    aliases: &HashMap<String, String>,
    known_values: &HashMap<String, Vec<String>>,
    files: &[FileSnapshot],
) -> Vec<String> {
    if edge.protocol != CommunicationProtocol::Grpc {
        return vec![edge.destination.clone()];
    }
    let Some((receiver, method)) = edge.destination.split_once('/') else {
        return vec![edge.destination.clone()];
    };
    let resolved = values_for(
        Some(receiver),
        bindings,
        aliases,
        known_values,
        callable,
        files,
    )
    .into_iter()
    .flatten()
    .filter_map(|value| client_service_name(&value).map(|service| format!("{service}/{method}")))
    .collect::<std::collections::HashSet<_>>();
    if resolved.is_empty() {
        vec![
            fallback_grpc_destination(receiver, method).unwrap_or_else(|| edge.destination.clone()),
        ]
    } else {
        resolved.into_iter().collect()
    }
}

/// Retains the extractor's conservative naming fallback when a receiver field cannot be
/// traced to a constructor (for example, an externally initialized client).
fn fallback_grpc_destination(receiver: &str, method: &str) -> Option<String> {
    let field = receiver.rsplit('.').next()?;
    let service = field.strip_suffix("Client")?;
    (!service.is_empty()).then(|| {
        let mut characters = service.chars();
        let first = characters
            .next()
            .unwrap_or_default()
            .to_uppercase()
            .to_string();
        format!("{first}{}Service/{method}", characters.as_str())
    })
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
