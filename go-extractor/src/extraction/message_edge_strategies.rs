use models::{CallStatement, MessageEdge};

use super::{kafka_message_edges, message_edges};

/// Identifies message edges for one transport family from a Go call statement.
pub(super) trait MessageEdgeIdentificationStrategy: Sync {
    fn identify(&self, call: &CallStatement, file_path: &str) -> Vec<MessageEdge>;
}

struct RabbitMqStrategy;
struct KafkaStrategy;

static RABBIT_MQ: RabbitMqStrategy = RabbitMqStrategy;
static KAFKA: KafkaStrategy = KafkaStrategy;
static STRATEGIES: &[&dyn MessageEdgeIdentificationStrategy] = &[&RABBIT_MQ, &KAFKA];

pub(super) fn identify_message_edges(call: &CallStatement, file_path: &str) -> Vec<MessageEdge> {
    STRATEGIES
        .iter()
        .flat_map(|strategy| strategy.identify(call, file_path))
        .collect()
}

impl MessageEdgeIdentificationStrategy for RabbitMqStrategy {
    fn identify(&self, call: &CallStatement, file_path: &str) -> Vec<MessageEdge> {
        message_edges::identify_message_edge(call, file_path)
            .into_iter()
            .collect()
    }
}

impl MessageEdgeIdentificationStrategy for KafkaStrategy {
    fn identify(&self, call: &CallStatement, file_path: &str) -> Vec<MessageEdge> {
        kafka_message_edges::identify_message_edges(call, file_path)
    }
}
