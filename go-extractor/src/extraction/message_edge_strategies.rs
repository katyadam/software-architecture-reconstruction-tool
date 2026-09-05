use std::collections::HashMap;

use models::{CallStatement, MessageEdge, ir::project::TypedFileRecord};

use super::{kafka_message_edges, message_edges, shared::merged_scope_bindings};

pub(super) struct MessageEdgeContext<'a> {
    pub call: &'a CallStatement,
    pub file_path: &'a str,
    pub scope: HashMap<String, String>,
    pub is_kafka_file: bool,
}

/// Identifies message edges for one transport family from a Go call statement.
pub(super) trait MessageEdgeIdentificationStrategy: Sync {
    fn identify(&self, ctx: &MessageEdgeContext<'_>) -> Vec<MessageEdge>;
}

struct RabbitMqStrategy;
struct KafkaStrategy;

static RABBIT_MQ: RabbitMqStrategy = RabbitMqStrategy;
static KAFKA: KafkaStrategy = KafkaStrategy;
static STRATEGIES: &[&dyn MessageEdgeIdentificationStrategy] = &[&RABBIT_MQ, &KAFKA];

pub(super) fn identify_message_edges(
    file: &TypedFileRecord,
    call: &CallStatement,
) -> Vec<MessageEdge> {
    let scope = call
        .enclosing_function_name
        .as_ref()
        .map(|name| models::Scope::Function(name.clone()))
        .unwrap_or(models::Scope::Global);
    let ctx = MessageEdgeContext {
        call,
        file_path: &file.file_path,
        scope: merged_scope_bindings(&file.assignments, &scope),
        is_kafka_file: file
            .imports
            .iter()
            .any(|import| import.orig_module.to_ascii_lowercase().contains("kafka")),
    };
    STRATEGIES
        .iter()
        .flat_map(|strategy| strategy.identify(&ctx))
        .collect()
}

impl MessageEdgeIdentificationStrategy for RabbitMqStrategy {
    fn identify(&self, ctx: &MessageEdgeContext<'_>) -> Vec<MessageEdge> {
        message_edges::identify_message_edge(ctx.call, ctx.file_path, &ctx.scope)
            .into_iter()
            .collect()
    }
}

impl MessageEdgeIdentificationStrategy for KafkaStrategy {
    fn identify(&self, ctx: &MessageEdgeContext<'_>) -> Vec<MessageEdge> {
        kafka_message_edges::identify_message_edges(
            ctx.call,
            ctx.file_path,
            &ctx.scope,
            ctx.is_kafka_file,
        )
    }
}
