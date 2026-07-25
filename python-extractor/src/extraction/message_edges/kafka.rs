use models::{Argument, CommunicationProtocol, MessageDestinationKind, MessageEdge, MessageRole};

use crate::extraction::calls::PythonCallStatement;

#[derive(Default)]
pub struct KafkaIdentificationStrategy;

impl KafkaIdentificationStrategy {
    pub fn new() -> Self {
        Self {}
    }

    pub fn identify_message_edges(
        &self,
        call: &PythonCallStatement,
        file_path: &str,
    ) -> Vec<MessageEdge> {
        let function_name = call.call_statement.function_name.as_str();
        let method = function_name.split('.').last().unwrap_or(function_name);

        match method {
            "produce" | "send" | "send_and_wait" => self
                .identify_producer(call, file_path)
                .into_iter()
                .collect(),
            "subscribe" | "subscriber" => self.identify_consumer_topic_arg(call, file_path),
            "publisher" => self
                .identify_producer(call, file_path)
                .into_iter()
                .collect(),
            "KafkaConsumer" | "AIOKafkaConsumer" => {
                self.identify_consumer_constructor(call, file_path)
            }
            "topic" => self
                .identify_quix_topic(call, file_path)
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }

    fn identify_producer(
        &self,
        call: &PythonCallStatement,
        file_path: &str,
    ) -> Option<MessageEdge> {
        let topic = get_arg(&call.call_statement.arguments, "topic", 0).map(clean_topic)?;
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

    fn identify_consumer_topic_arg(
        &self,
        call: &PythonCallStatement,
        file_path: &str,
    ) -> Vec<MessageEdge> {
        get_arg(&call.call_statement.arguments, "topics", 0)
            .or_else(|| get_arg(&call.call_statement.arguments, "topic", 0))
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

    fn identify_consumer_constructor(
        &self,
        call: &PythonCallStatement,
        file_path: &str,
    ) -> Vec<MessageEdge> {
        call.call_statement
            .arguments
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

    fn identify_quix_topic(
        &self,
        call: &PythonCallStatement,
        file_path: &str,
    ) -> Option<MessageEdge> {
        let topic = get_arg(&call.call_statement.arguments, "topic", 0).map(clean_topic)?;
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

    fn edge(
        &self,
        role: MessageRole,
        destination: String,
        handler: Option<String>,
        topic: Option<String>,
        call: &PythonCallStatement,
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
            function_name: call
                .call_statement
                .enclosing_function_name
                .clone()
                .unwrap_or_default(),
            function_hash: call
                .call_statement
                .enclosing_function_hash
                .clone()
                .unwrap_or_default(),
            call_arguments: call.call_statement.arguments.clone(),
            file_path: file_path.to_string(),
        }
    }
}

fn get_arg<'a>(arguments: &'a [Argument], name: &str, index: usize) -> Option<&'a str> {
    arguments
        .iter()
        .find(|arg| arg.assigned_variable == name)
        .or_else(|| arguments.get(index))
        .map(|arg| arg.value.as_str())
}

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

fn clean_topic(raw: &str) -> String {
    statix::strings::clean_python_string(raw)
}

fn topic_is_payloadish(topic: &str) -> bool {
    matches!(
        topic,
        "data" | "message" | "payload" | "value" | "body" | "label"
    )
}
