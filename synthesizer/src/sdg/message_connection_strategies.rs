use std::collections::HashMap;

use models::{CommunicationProtocol, MessageDestinationKind, MessageRole};

use super::model::{AssignedMessageEdge, MessageConnection, MessageRequest};

/// Builds message connections using ordered transport-aware strategies.
pub(super) fn create(message_edges: Vec<AssignedMessageEdge>) -> Vec<MessageConnection> {
    let strategies: &[&dyn MessageConnectionStrategy] = &[
        &RabbitMqTopologyStrategy,
        &DirectMessageConnectionStrategy,
        &BindingFallbackStrategy,
    ];
    let mut connections = HashMap::new();
    for strategy in strategies {
        for (producer, consumer) in strategy.connections(&message_edges) {
            insert(&mut connections, producer, consumer);
        }
    }
    connections.into_values().collect()
}

/// Defines one way to connect producer edges to destination edges.
trait MessageConnectionStrategy: Sync {
    /// Returns every producer and destination pair recognized by this strategy.
    fn connections<'a>(
        &self,
        edges: &'a [AssignedMessageEdge],
    ) -> Vec<(&'a AssignedMessageEdge, &'a AssignedMessageEdge)>;
}

struct RabbitMqTopologyStrategy;
struct DirectMessageConnectionStrategy;
struct BindingFallbackStrategy;

impl MessageConnectionStrategy for RabbitMqTopologyStrategy {
    /// Links publishers through centralized RabbitMQ bindings to actual queue consumers.
    fn connections<'a>(
        &self,
        edges: &'a [AssignedMessageEdge],
    ) -> Vec<(&'a AssignedMessageEdge, &'a AssignedMessageEdge)> {
        let producers = edges.iter().filter(|edge| {
            edge.data.protocol == CommunicationProtocol::RabbitMq
                && edge.data.role == MessageRole::Producer
        });
        let bindings = edges.iter().filter(|edge| {
            edge.data.protocol == CommunicationProtocol::RabbitMq
                && edge.data.role == MessageRole::Binding
        });
        let consumers = edges.iter().filter(|edge| {
            edge.data.protocol == CommunicationProtocol::RabbitMq
                && edge.data.role == MessageRole::Consumer
        });
        let mut pairs = Vec::new();
        for producer in producers {
            for binding in bindings
                .clone()
                .filter(|binding| matches(producer, binding))
            {
                for consumer in consumers
                    .clone()
                    .filter(|consumer| queue_matches(binding, consumer))
                {
                    if producer.service.name != consumer.service.name {
                        pairs.push((producer, consumer));
                    }
                }
            }
        }
        pairs
    }
}

impl MessageConnectionStrategy for DirectMessageConnectionStrategy {
    /// Links messages directly when a consumer exposes the transport destination itself.
    fn connections<'a>(
        &self,
        edges: &'a [AssignedMessageEdge],
    ) -> Vec<(&'a AssignedMessageEdge, &'a AssignedMessageEdge)> {
        let consumers = edges
            .iter()
            .filter(|edge| edge.data.role == MessageRole::Consumer)
            .collect::<Vec<_>>();
        edges
            .iter()
            .filter(|edge| edge.data.role == MessageRole::Producer)
            .flat_map(|producer| {
                consumers.iter().filter_map(move |consumer| {
                    (producer.service.name != consumer.service.name && matches(producer, consumer))
                        .then_some((producer, *consumer))
                })
            })
            .collect()
    }
}

impl MessageConnectionStrategy for BindingFallbackStrategy {
    /// Uses a binding's declaring service only when no queue consumer is known.
    fn connections<'a>(
        &self,
        edges: &'a [AssignedMessageEdge],
    ) -> Vec<(&'a AssignedMessageEdge, &'a AssignedMessageEdge)> {
        let consumers = edges
            .iter()
            .filter(|edge| edge.data.role == MessageRole::Consumer)
            .collect::<Vec<_>>();
        let bindings = edges
            .iter()
            .filter(|edge| edge.data.role == MessageRole::Binding)
            .collect::<Vec<_>>();
        edges
            .iter()
            .filter(|edge| edge.data.role == MessageRole::Producer)
            .flat_map(|producer| {
                let consumers = consumers.clone();
                bindings.iter().filter_map(move |binding| {
                    (producer.service.name != binding.service.name
                        && matches(producer, binding)
                        && !consumers
                            .iter()
                            .any(|consumer| queue_matches(binding, consumer)))
                    .then_some((producer, *binding))
                })
            })
            .collect()
    }
}

/// Inserts a matched edge pair, deduplicating repeated strategy results.
fn insert(
    connections: &mut HashMap<String, MessageConnection>,
    producer: &AssignedMessageEdge,
    consumer: &AssignedMessageEdge,
) {
    let connection = connections
        .entry(format!(
            "{}__{}",
            producer.service.name, consumer.service.name
        ))
        .or_insert_with(|| MessageConnection {
            source_id: producer.service.name.clone(),
            target_id: consumer.service.name.clone(),
            messages: Vec::new(),
        });
    let request = MessageRequest {
        producer: producer.data.clone(),
        consumer: consumer.data.clone(),
    };
    if !connection.messages.contains(&request) {
        connection.messages.push(request);
    }
}

/// Checks that a binding queue and consumer queue are the same static value.
fn queue_matches(binding: &AssignedMessageEdge, consumer: &AssignedMessageEdge) -> bool {
    binding.data.queue.as_ref().is_some_and(|queue| {
        consumer
            .data
            .queue
            .as_ref()
            .is_some_and(|consumer_queue| consumer_queue == queue)
    })
}

/// Compares protocol destinations, including fanout exchanges without routing keys.
fn matches(producer: &AssignedMessageEdge, destination: &AssignedMessageEdge) -> bool {
    let producer = &producer.data;
    let destination = &destination.data;
    match producer.destination_kind {
        MessageDestinationKind::Topic => producer.topic == destination.topic,
        MessageDestinationKind::Queue => producer.queue == destination.queue,
        MessageDestinationKind::ExchangeRoutingKey => {
            producer.exchange == destination.exchange
                && producer.routing_key == destination.routing_key
        }
        MessageDestinationKind::Unknown => false,
    }
}
