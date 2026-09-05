use std::collections::HashMap;

use models::{
    CallStatement, CommunicationProtocol, MessageDestinationKind, MessageEdge, MessageRole,
};

const PRODUCER_METHODS: &[&str] = &["WriteMessages", "Produce", "ProduceSync", "SendMessage"];
const CONSUMER_METHODS: &[&str] = &["Subscribe", "SubscribeTopics", "ConsumePartition"];

pub(super) fn identify_message_edges(
    call: &CallStatement,
    file_path: &str,
    scope: &HashMap<String, String>,
    is_kafka_file: bool,
) -> Vec<MessageEdge> {
    let Some(method) = call.function_name.rsplit('.').next() else {
        return Vec::new();
    };

    match method {
        method if PRODUCER_METHODS.contains(&method) => {
            producer_from_record(call, file_path, scope)
        }
        "NewWriter" => producer_from_config(call, file_path, scope),
        method if CONSUMER_METHODS.contains(&method) => {
            consumer_from_argument(call, file_path, scope, 0)
        }
        "NewReader" => consumer_from_config(call, file_path, scope),
        // Sarama ConsumerGroup.Consume(ctx, []string{"topic"}, handler).
        "Consume" if call.arguments.len() > 1 => consumer_from_argument(call, file_path, scope, 1),
        // franz-go uses SeedTopics to configure the topics consumed by a client.
        "SeedTopics" => consumer_from_argument(call, file_path, scope, 0),
        // Application wrappers commonly expose the Kafka topic as the third
        // Producer argument and the first Consumer argument.
        "Producer" if is_kafka_file && call.arguments.len() > 2 => {
            producer_from_argument(call, file_path, scope, 2)
        }
        "Consumer" if is_kafka_file => consumer_from_argument(call, file_path, scope, 0),
        _ => Vec::new(),
    }
}

fn producer_from_argument(
    call: &CallStatement,
    file_path: &str,
    scope: &HashMap<String, String>,
    index: usize,
) -> Vec<MessageEdge> {
    call.arguments
        .get(index)
        .into_iter()
        .flat_map(|argument| topics_from_field_or_literals(&argument.value, "Topic", scope))
        .map(|topic| edge(MessageRole::Producer, topic, call, file_path))
        .collect()
}

fn producer_from_record(
    call: &CallStatement,
    file_path: &str,
    scope: &HashMap<String, String>,
) -> Vec<MessageEdge> {
    call.arguments
        .iter()
        // Producer APIs may take a context before their message records.
        // Only inline message records can provide a statically known topic.
        .map(|argument| resolve_value(&argument.value, scope))
        .filter(|argument| argument.contains('{'))
        .flat_map(|argument| topics_from_record(&argument, scope))
        .map(|topic| edge(MessageRole::Producer, topic, call, file_path))
        .collect()
}

fn producer_from_config(
    call: &CallStatement,
    file_path: &str,
    scope: &HashMap<String, String>,
) -> Vec<MessageEdge> {
    call.arguments
        .iter()
        .flat_map(|argument| topics_from_field_or_literals(&argument.value, "Topic", scope))
        .map(|topic| edge(MessageRole::Producer, topic, call, file_path))
        .collect()
}

fn consumer_from_config(
    call: &CallStatement,
    file_path: &str,
    scope: &HashMap<String, String>,
) -> Vec<MessageEdge> {
    call.arguments
        .iter()
        .flat_map(|argument| topics_from_field_or_literals(&argument.value, "Topic", scope))
        .map(|topic| edge(MessageRole::Consumer, topic, call, file_path))
        .collect()
}

fn consumer_from_argument(
    call: &CallStatement,
    file_path: &str,
    scope: &HashMap<String, String>,
    index: usize,
) -> Vec<MessageEdge> {
    call.arguments
        .get(index)
        .into_iter()
        .flat_map(|argument| topics_from_field_or_literals(&argument.value, "Topic", scope))
        .map(|topic| edge(MessageRole::Consumer, topic, call, file_path))
        .collect()
}

fn topics_from_record(raw: &str, scope: &HashMap<String, String>) -> Vec<String> {
    topics_from_field_or_literals(raw, "Topic", scope)
}

fn topics_from_field_or_literals(
    raw: &str,
    field: &str,
    scope: &HashMap<String, String>,
) -> Vec<String> {
    let raw = resolve_value(raw, scope);
    let field_value = field_value(&raw, field).map(|value| resolve_value(&value, scope));
    let mut topics = field_value
        .as_deref()
        .map(string_literals)
        .unwrap_or_default();
    if topics.is_empty() {
        if let Some(value) = field_value {
            let candidate = clean_topic(&value);
            if !candidate.is_empty() && !candidate.contains(['{', '}', '[', ']']) {
                topics.push(candidate);
            }
        }
    }
    if topics.is_empty() {
        topics = string_literals(&raw);
    }
    if topics.is_empty() {
        let candidate = clean_topic(&raw);
        if !candidate.is_empty() && !candidate.contains(['{', '}', '[', ']']) {
            topics.push(candidate);
        }
    }
    topics.sort();
    topics.dedup();
    topics.retain(|topic| !is_unresolved_identifier(topic, scope));
    topics
}

fn is_unresolved_identifier(value: &str, scope: &HashMap<String, String>) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
        && !scope.contains_key(value)
}

fn resolve_value(raw: &str, scope: &HashMap<String, String>) -> String {
    super::shared::evaluate_expression_text(raw, scope)
}

fn field_value(raw: &str, field: &str) -> Option<String> {
    let start = raw.find(&format!("{field}:"))? + field.len() + 1;
    let value = raw[start..].trim_start();
    let mut nesting = 0_u32;
    let mut end = value.len();
    for (index, character) in value.char_indices() {
        match character {
            '(' | '{' | '[' => nesting += 1,
            ')' | '}' | ']' if nesting == 0 => {
                end = index;
                break;
            }
            ')' | '}' | ']' => nesting -= 1,
            ',' if nesting == 0 => {
                end = index;
                break;
            }
            _ => {}
        }
    }
    Some(value[..end].trim().to_string())
}

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
                let value = raw[start..index].to_string();
                if !value.is_empty() {
                    values.push(value);
                }
                index += 1;
                break;
            }
            index += 1;
        }
    }
    values
}

fn clean_topic(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('&')
        .trim_matches('"')
        .trim_matches('`')
        .to_string()
}

fn edge(role: MessageRole, topic: String, call: &CallStatement, file_path: &str) -> MessageEdge {
    MessageEdge {
        protocol: CommunicationProtocol::Kafka,
        role,
        destination_kind: MessageDestinationKind::Topic,
        destination: topic.clone(),
        exchange: None,
        routing_key: None,
        queue: None,
        topic: Some(topic),
        handler: None,
        function_name: call.enclosing_function_name.clone().unwrap_or_default(),
        function_hash: call.enclosing_function_hash.clone().unwrap_or_default(),
        call_arguments: call.arguments.clone(),
        file_path: file_path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::topics_from_field_or_literals;

    #[test]
    fn extracts_topics_from_kafka_record_and_lists() {
        assert_eq!(
            topics_from_field_or_literals(
                "&kafka.Message{TopicPartition: kafka.TopicPartition{Topic: strPtr(\"orders.created\")}}",
                "Topic",
                &HashMap::new()
            ),
            vec!["orders.created"]
        );
        assert_eq!(
            topics_from_field_or_literals(
                "[]string{\"orders.created\", \"orders.retry\"}",
                "Topic",
                &HashMap::new()
            ),
            vec!["orders.created", "orders.retry"]
        );
    }

    #[test]
    fn rejects_unresolved_topic_identifiers() {
        assert!(topics_from_field_or_literals("topic", "Topic", &HashMap::new()).is_empty());
        assert_eq!(
            topics_from_field_or_literals(
                "topic",
                "Topic",
                &HashMap::from([(String::from("topic"), String::from("orders.created"))])
            ),
            vec!["orders.created"]
        );
    }
}
