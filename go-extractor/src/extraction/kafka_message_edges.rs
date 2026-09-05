use models::{
    CallStatement, CommunicationProtocol, MessageDestinationKind, MessageEdge, MessageRole,
};

const PRODUCER_METHODS: &[&str] = &["WriteMessages", "Produce", "ProduceSync", "SendMessage"];
const CONSUMER_METHODS: &[&str] = &["Subscribe", "SubscribeTopics", "ConsumePartition"];

pub(super) fn identify_message_edges(call: &CallStatement, file_path: &str) -> Vec<MessageEdge> {
    let Some(method) = call.function_name.rsplit('.').next() else {
        return Vec::new();
    };

    match method {
        method if PRODUCER_METHODS.contains(&method) => producer_from_record(call, file_path),
        "NewWriter" => producer_from_config(call, file_path),
        method if CONSUMER_METHODS.contains(&method) => consumer_from_argument(call, file_path, 0),
        "NewReader" => consumer_from_config(call, file_path),
        // Sarama ConsumerGroup.Consume(ctx, []string{"topic"}, handler).
        "Consume" if call.arguments.len() > 1 => consumer_from_argument(call, file_path, 1),
        // franz-go uses SeedTopics to configure the topics consumed by a client.
        "SeedTopics" => consumer_from_argument(call, file_path, 0),
        _ => Vec::new(),
    }
}

fn producer_from_record(call: &CallStatement, file_path: &str) -> Vec<MessageEdge> {
    call.arguments
        .iter()
        .flat_map(|argument| topics_from_record(&argument.value))
        .map(|topic| edge(MessageRole::Producer, topic, call, file_path))
        .collect()
}

fn producer_from_config(call: &CallStatement, file_path: &str) -> Vec<MessageEdge> {
    call.arguments
        .iter()
        .flat_map(|argument| topics_from_field_or_literals(&argument.value, "Topic"))
        .map(|topic| edge(MessageRole::Producer, topic, call, file_path))
        .collect()
}

fn consumer_from_config(call: &CallStatement, file_path: &str) -> Vec<MessageEdge> {
    call.arguments
        .iter()
        .flat_map(|argument| topics_from_field_or_literals(&argument.value, "Topic"))
        .map(|topic| edge(MessageRole::Consumer, topic, call, file_path))
        .collect()
}

fn consumer_from_argument(call: &CallStatement, file_path: &str, index: usize) -> Vec<MessageEdge> {
    call.arguments
        .get(index)
        .into_iter()
        .flat_map(|argument| topics_from_field_or_literals(&argument.value, "Topic"))
        .map(|topic| edge(MessageRole::Consumer, topic, call, file_path))
        .collect()
}

fn topics_from_record(raw: &str) -> Vec<String> {
    topics_from_field_or_literals(raw, "Topic")
}

fn topics_from_field_or_literals(raw: &str, field: &str) -> Vec<String> {
    let mut topics = field_value(raw, field)
        .map(|value| string_literals(&value))
        .unwrap_or_default();
    if topics.is_empty() {
        topics = string_literals(raw);
    }
    if topics.is_empty() {
        let candidate = clean_topic(raw);
        if !candidate.is_empty() && !candidate.contains(['{', '}', '[', ']']) {
            topics.push(candidate);
        }
    }
    topics.sort();
    topics.dedup();
    topics
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
    use super::topics_from_field_or_literals;

    #[test]
    fn extracts_topics_from_kafka_record_and_lists() {
        assert_eq!(
            topics_from_field_or_literals(
                "&kafka.Message{TopicPartition: kafka.TopicPartition{Topic: strPtr(\"orders.created\")}}",
                "Topic"
            ),
            vec!["orders.created"]
        );
        assert_eq!(
            topics_from_field_or_literals(
                "[]string{\"orders.created\", \"orders.retry\"}",
                "Topic"
            ),
            vec!["orders.created", "orders.retry"]
        );
    }
}
