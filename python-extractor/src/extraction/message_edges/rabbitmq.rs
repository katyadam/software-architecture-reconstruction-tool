use models::{
    Argument, CallStatement, CommunicationProtocol, MessageDestinationKind, MessageEdge,
    MessageRole,
};

const RABBITMQ_PUBLISH_METHOD: &str = "basic_publish";
const RABBITMQ_CONSUME_METHOD: &str = "basic_consume";
const RABBITMQ_QUEUE_DECLARE_METHOD: &str = "queue_declare";
const RABBITMQ_QUEUE_BIND_METHOD: &str = "queue_bind";
const METHOD_SEPARATOR: char = '.';
const DESTINATION_SEPARATOR: &str = ":";
const EXCHANGE_ARGUMENT: &str = "exchange";
const ROUTING_KEY_ARGUMENT: &str = "routing_key";
const QUEUE_ARGUMENT: &str = "queue";
const MESSAGE_CALLBACK_ARGUMENT: &str = "on_message_callback";

#[derive(Default)]
pub struct RabbitMqIdentificationStrategy;

impl RabbitMqIdentificationStrategy {
    /// Creates a RabbitMQ message-edge identification strategy.
    pub fn new() -> Self {
        Self {}
    }

    /// Identifies supported RabbitMQ calls by their method names.
    pub fn identify_message_edge(
        &self,
        call: &CallStatement,
        file_path: &str,
    ) -> Option<MessageEdge> {
        let method = call.function_name.split(METHOD_SEPARATOR).last()?;
        match method {
            RABBITMQ_PUBLISH_METHOD => self.identify_publish(call, file_path),
            RABBITMQ_CONSUME_METHOD => self.identify_consume(call, file_path),
            RABBITMQ_QUEUE_DECLARE_METHOD => self.identify_queue_declare(call, file_path),
            RABBITMQ_QUEUE_BIND_METHOD => self.identify_queue_bind(call, file_path),
            _ => None,
        }
    }

    /// Extracts an exchange and routing key from a publish call.
    fn identify_publish(&self, call: &CallStatement, file_path: &str) -> Option<MessageEdge> {
        let exchange = get_arg(&call.arguments, EXCHANGE_ARGUMENT, 0)
            .map(clean_value)
            .filter(|v| !v.is_empty());
        let routing_key = get_arg(&call.arguments, ROUTING_KEY_ARGUMENT, 1).map(clean_value);

        let is_default_exchange = exchange.is_none();
        let destination_kind = if is_default_exchange {
            MessageDestinationKind::Queue
        } else {
            MessageDestinationKind::ExchangeRoutingKey
        };

        let destination = match (&exchange, &routing_key) {
            (Some(exchange), Some(routing_key)) => {
                format!("{exchange}{DESTINATION_SEPARATOR}{routing_key}")
            }
            (Some(exchange), None) => exchange.clone(),
            (None, Some(routing_key)) => routing_key.clone(),
            (None, None) => return None,
        };

        Some(self.edge(
            MessageRole::Producer,
            destination_kind,
            destination,
            exchange,
            routing_key.clone(),
            routing_key.filter(|_| is_default_exchange),
            None,
            call,
            file_path,
        ))
    }

    /// Extracts the queue and callback from a consumer call.
    fn identify_consume(&self, call: &CallStatement, file_path: &str) -> Option<MessageEdge> {
        let queue = get_arg(&call.arguments, QUEUE_ARGUMENT, 0).map(clean_value)?;
        let handler = get_arg(&call.arguments, MESSAGE_CALLBACK_ARGUMENT, 1).map(clean_value);

        Some(self.edge(
            MessageRole::Consumer,
            MessageDestinationKind::Queue,
            queue.clone(),
            None,
            None,
            Some(queue),
            handler,
            call,
            file_path,
        ))
    }

    /// Extracts the queue declared by a queue-declaration call.
    fn identify_queue_declare(&self, call: &CallStatement, file_path: &str) -> Option<MessageEdge> {
        let queue = get_arg(&call.arguments, QUEUE_ARGUMENT, 0).map(clean_value)?;

        Some(self.edge(
            MessageRole::QueueDeclaration,
            MessageDestinationKind::Queue,
            queue.clone(),
            None,
            None,
            Some(queue),
            None,
            call,
            file_path,
        ))
    }

    fn identify_queue_bind(&self, call: &CallStatement, file_path: &str) -> Option<MessageEdge> {
        let exchange = get_arg(&call.arguments, EXCHANGE_ARGUMENT, 0)
            .map(clean_value)
            .filter(|v| !v.is_empty());
        let queue = get_arg(&call.arguments, QUEUE_ARGUMENT, 1).map(clean_value);
        let routing_key = get_arg(&call.arguments, ROUTING_KEY_ARGUMENT, 2).map(clean_value);

        let destination = match (&exchange, &routing_key) {
            (Some(exchange), Some(routing_key)) => {
                format!("{exchange}{DESTINATION_SEPARATOR}{routing_key}")
            }
            (Some(exchange), None) => exchange.clone(),
            (None, Some(routing_key)) => routing_key.clone(),
            (None, None) => queue.clone().unwrap_or_default(),
        };
        if destination.is_empty() {
            return None;
        }

        Some(self.edge(
            MessageRole::Binding,
            if exchange.is_some() {
                MessageDestinationKind::ExchangeRoutingKey
            } else {
                MessageDestinationKind::Queue
            },
            destination,
            exchange,
            routing_key,
            queue,
            None,
            call,
            file_path,
        ))
    }

    /// Builds a RabbitMQ message edge with call-site metadata.
    fn edge(
        &self,
        role: MessageRole,
        destination_kind: MessageDestinationKind,
        destination: String,
        exchange: Option<String>,
        routing_key: Option<String>,
        queue: Option<String>,
        handler: Option<String>,
        call: &CallStatement,
        file_path: &str,
    ) -> MessageEdge {
        MessageEdge {
            protocol: CommunicationProtocol::RabbitMq,
            role,
            destination_kind,
            destination,
            topic: None,
            exchange,
            routing_key,
            queue,
            handler,
            function_name: call.enclosing_function_name.clone().unwrap_or_default(),
            function_hash: call.enclosing_function_hash.clone().unwrap_or_default(),
            call_arguments: call.arguments.clone(),
            file_path: file_path.to_string(),
        }
    }
}

/// Finds a named argument, falling back to its positional index.
fn get_arg<'a>(arguments: &'a [Argument], name: &str, index: usize) -> Option<&'a str> {
    arguments
        .iter()
        .find(|arg| arg.assigned_variable == name)
        .or_else(|| arguments.get(index))
        .map(|arg| arg.value.as_str())
}

/// Removes Python string syntax from a RabbitMQ value.
fn clean_value(raw: &str) -> String {
    statix::strings::clean_python_string(raw)
}
