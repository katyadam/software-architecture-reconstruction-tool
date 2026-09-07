use std::collections::HashMap;

use models::{CallStatement, MessageEdge, ir::project::TypedFileRecord};

use super::{kafka_message_edges, message_edges, shared::merged_scope_bindings};

pub(super) struct MessageEdgeContext<'a> {
    pub call: &'a CallStatement,
    pub file_path: &'a str,
    pub scope: HashMap<String, String>,
    pub is_kafka_file: bool,
    pub is_rabbitmq_file: bool,
}

/// Identifies message edges for one transport family from a Go call statement.
pub(super) trait MessageEdgeIdentificationStrategy: Sync {
    /// Returns every edge recognized for the call context.
    fn identify(&self, ctx: &MessageEdgeContext<'_>) -> Vec<MessageEdge>;
}

struct RabbitMqStrategy;
struct RabbitMqConfigurationBuilderStrategy;
struct KafkaStrategy;

static RABBIT_MQ: RabbitMqStrategy = RabbitMqStrategy;
static RABBIT_MQ_CONFIGURATION_BUILDER: RabbitMqConfigurationBuilderStrategy =
    RabbitMqConfigurationBuilderStrategy;
static KAFKA: KafkaStrategy = KafkaStrategy;
static STRATEGIES: &[&dyn MessageEdgeIdentificationStrategy] =
    &[&RABBIT_MQ, &RABBIT_MQ_CONFIGURATION_BUILDER, &KAFKA];

/// Builds call context and runs every transport-specific identification strategy.
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
        is_kafka_file: file.imports.iter().any(|import| {
            let module = import.orig_module.to_ascii_lowercase();
            module.contains("kafka") || module.contains("sarama")
        }),
        is_rabbitmq_file: file.imports.iter().any(|import| {
            let module = import.orig_module.to_ascii_lowercase();
            module.contains("rabbitmq") || module.contains("amqp")
        }),
    };
    STRATEGIES
        .iter()
        .flat_map(|strategy| strategy.identify(&ctx))
        .collect()
}

impl MessageEdgeIdentificationStrategy for RabbitMqStrategy {
    /// Delegates a call to the RabbitMQ edge recognizer.
    fn identify(&self, ctx: &MessageEdgeContext<'_>) -> Vec<MessageEdge> {
        let method = ctx.call.function_name.rsplit('.').next();
        // Queue declarations and bindings are specific to AMQP. Publish and
        // Consume overlap with other transports, so require an AMQP import.
        if !ctx.is_rabbitmq_file
            && matches!(method, Some("Publish" | "PublishWithContext" | "Consume"))
        {
            return Vec::new();
        }
        message_edges::identify_message_edge(ctx.call, ctx.file_path, &ctx.scope)
            .into_iter()
            .collect()
    }
}

impl MessageEdgeIdentificationStrategy for RabbitMqConfigurationBuilderStrategy {
    /// Recognizes the food-delivery RabbitMQ configuration builder contract.
    fn identify(&self, ctx: &MessageEdgeContext<'_>) -> Vec<MessageEdge> {
        let is_library_producer = ctx
            .call
            .function_name
            .to_ascii_lowercase()
            .contains("rabbitmqproducer.publishmessage");
        if !ctx.is_rabbitmq_file && !is_library_producer {
            return Vec::new();
        }
        message_edges::identify_configuration_builder_edges(ctx.call, ctx.file_path)
    }
}

impl MessageEdgeIdentificationStrategy for KafkaStrategy {
    /// Delegates a call to the Kafka edge recognizer with import provenance.
    fn identify(&self, ctx: &MessageEdgeContext<'_>) -> Vec<MessageEdge> {
        kafka_message_edges::identify_message_edges(
            ctx.call,
            ctx.file_path,
            &ctx.scope,
            ctx.is_kafka_file,
        )
    }
}
