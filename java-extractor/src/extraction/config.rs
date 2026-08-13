use std::collections::HashMap;

use models::{
    CommunicationProtocol, MessageDestinationKind, MessageEdge, MessageRole,
    ir::{language::Language, syntax::FileRecord},
};

/// Extracts Java ecosystem message bindings from configuration files.
///
/// The runtime delegates configuration files to the Java extractor because
/// these bindings are part of Spring Cloud Stream's Java messaging model.
/// Framework-specific syntax remains encapsulated in this crate.
pub fn extract_syntactic(text: &str, file_path: &str) -> Option<FileRecord> {
    let raw_message_edges = text
        .lines()
        .filter_map(|line| spring_cloud_stream_binding_edge(line, file_path))
        .collect::<Vec<_>>();

    if raw_message_edges.is_empty() {
        return None;
    }

    Some(FileRecord {
        file_path: file_path.to_string(),
        language: Language::Java,
        imports: Vec::new(),
        entities: Vec::new(),
        endpoints: Vec::new(),
        callables: Vec::new(),
        call_statements: Vec::new(),
        assignments: HashMap::new(),
        enums: Vec::new(),
        raw_restcalls: Vec::new(),
        raw_message_edges,
        proto_services: Vec::new(),
    })
}

fn spring_cloud_stream_binding_edge(line: &str, file_path: &str) -> Option<MessageEdge> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let (key, value) = trimmed
        .split_once(':')
        .or_else(|| trimmed.split_once('='))?;
    let key = key.trim();
    let binding = key
        .strip_prefix("spring.cloud.stream.bindings.")?
        .strip_suffix(".destination")?;
    let role = if binding.contains("-out-") {
        MessageRole::Producer
    } else if binding.contains("-in-") {
        MessageRole::Consumer
    } else {
        return None;
    };

    let topic = clean_config_value(value);
    if topic.is_empty() {
        return None;
    }

    Some(MessageEdge {
        protocol: CommunicationProtocol::Kafka,
        role,
        destination_kind: MessageDestinationKind::Topic,
        destination: topic.clone(),
        exchange: None,
        routing_key: None,
        queue: None,
        topic: Some(topic),
        handler: Some(binding.to_string()),
        function_name: binding.to_string(),
        function_hash: String::new(),
        call_arguments: Vec::new(),
        file_path: file_path.to_string(),
    })
}

fn clean_config_value(raw: &str) -> String {
    raw.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}
