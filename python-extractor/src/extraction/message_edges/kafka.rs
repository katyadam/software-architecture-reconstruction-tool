use models::{
    Argument, CallStatement, CommunicationProtocol, MessageDestinationKind, MessageEdge,
    MessageRole,
};

const KAFKA_PRODUCER_METHODS: &[&str] = &["produce", "send", "send_and_wait", "publisher"];
const KAFKA_SUBSCRIPTION_METHODS: &[&str] = &["subscribe", "subscriber"];
const KAFKA_CONSUMER_CONSTRUCTORS: &[&str] = &["KafkaConsumer", "AIOKafkaConsumer"];
const KAFKA_TOPIC_METHOD: &str = "topic";
const PAYLOADISH_TOPIC_NAMES: &[&str] = &["data", "message", "payload", "value", "body", "label"];
const METHOD_SEPARATOR: char = '.';
const TOPIC_ARGUMENT: &str = "topic";
const TOPICS_ARGUMENT: &str = "topics";

#[derive(Default)]
pub struct KafkaIdentificationStrategy;

impl KafkaIdentificationStrategy {
    /// Creates a Kafka message-edge identification strategy.
    pub fn new() -> Self {
        Self {}
    }

    /// Identifies Kafka producer and consumer calls by their method names.
    pub fn identify_message_edges(
        &self,
        call: &CallStatement,
        file_path: &str,
    ) -> Vec<MessageEdge> {
        let function_name = call.function_name.as_str();
        let method = function_name
            .split(METHOD_SEPARATOR)
            .next_back()
            .unwrap_or(function_name);

        match method {
            method if KAFKA_PRODUCER_METHODS.contains(&method) => self
                .identify_producer(call, file_path)
                .into_iter()
                .collect(),
            method if KAFKA_SUBSCRIPTION_METHODS.contains(&method) => {
                self.identify_consumer_topic_arg(call, file_path)
            }
            method if KAFKA_CONSUMER_CONSTRUCTORS.contains(&method) => {
                self.identify_consumer_constructor(call, file_path)
            }
            KAFKA_TOPIC_METHOD => self
                .identify_quix_topic(call, file_path)
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Extracts a topic from a Kafka producer call.
    fn identify_producer(&self, call: &CallStatement, file_path: &str) -> Option<MessageEdge> {
        let topic = get_arg(&call.arguments, TOPIC_ARGUMENT, 0).map(clean_topic)?;
        if topic.is_empty() || topic_is_payloadish(&topic) {
            return None;
        }
        Some(self.edge(
            MessageRole::Producer,
            topic.clone(),
            None,
            Some(topic),
            call,
            file_path,
        ))
    }

    /// Extracts one or more topics from a subscription call.
    fn identify_consumer_topic_arg(
        &self,
        call: &CallStatement,
        file_path: &str,
    ) -> Vec<MessageEdge> {
        get_arg(&call.arguments, TOPICS_ARGUMENT, 0)
            .or_else(|| get_arg(&call.arguments, TOPIC_ARGUMENT, 0))
            .into_iter()
            .flat_map(clean_topics)
            .map(|topic| {
                self.edge(
                    MessageRole::Consumer,
                    topic.clone(),
                    None,
                    Some(topic),
                    call,
                    file_path,
                )
            })
            .collect()
    }

    /// Extracts positional topics from a Kafka consumer constructor.
    fn identify_consumer_constructor(
        &self,
        call: &CallStatement,
        file_path: &str,
    ) -> Vec<MessageEdge> {
        call.arguments
            .iter()
            .take_while(|arg| arg.assigned_variable.is_empty())
            .flat_map(|arg| clean_topics(arg.value.as_str()))
            .filter(|topic| !topic_is_payloadish(topic))
            .map(|topic| {
                self.edge(
                    MessageRole::Consumer,
                    topic.clone(),
                    None,
                    Some(topic),
                    call,
                    file_path,
                )
            })
            .collect()
    }

    /// Extracts the topic consumed by a Quix topic call.
    fn identify_quix_topic(&self, call: &CallStatement, file_path: &str) -> Option<MessageEdge> {
        let topic = get_arg(&call.arguments, TOPIC_ARGUMENT, 0).map(clean_topic)?;
        if topic.is_empty() {
            return None;
        }
        Some(self.edge(
            MessageRole::Consumer,
            topic.clone(),
            None,
            Some(topic),
            call,
            file_path,
        ))
    }

    /// Builds a Kafka message edge with call-site metadata.
    fn edge(
        &self,
        role: MessageRole,
        destination: String,
        handler: Option<String>,
        topic: Option<String>,
        call: &CallStatement,
        file_path: &str,
    ) -> MessageEdge {
        MessageEdge {
            protocol: CommunicationProtocol::Kafka,
            role,
            destination_kind: MessageDestinationKind::Topic,
            destination,
            topic,
            exchange: None,
            routing_key: None,
            queue: None,
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

/// Splits a topic list while preserving nested Python expressions.
fn clean_topics(raw: &str) -> Vec<String> {
    let cleaned = raw.trim();
    if cleaned.starts_with('[') && cleaned.ends_with(']') {
        return statix::strings::split_at_top_level(
            &cleaned[1..cleaned.len() - 1],
            &[','],
            &[('(', ')'), ('[', ']'), ('{', '}')],
        )
        .into_iter()
        .map(|topic| clean_topic(&topic))
        .filter(|topic| !topic.is_empty())
        .collect();
    }
    vec![clean_topic(cleaned)]
}

/// Removes Python string syntax from a topic value.
fn clean_topic(raw: &str) -> String {
    statix::strings::clean_python_string(raw)
}

/// Filters out common payload names that are unlikely to be topic values.
fn topic_is_payloadish(topic: &str) -> bool {
    PAYLOADISH_TOPIC_NAMES.contains(&topic)
}
