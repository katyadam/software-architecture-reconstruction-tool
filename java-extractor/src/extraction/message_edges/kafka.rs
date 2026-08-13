use std::{collections::HashMap, sync::OnceLock};

use models::{
    CallStatement, CommunicationProtocol, MessageDestinationKind, MessageEdge, MessageRole,
};
use regex::Regex;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator, Tree};

use crate::extraction::enclosing_lookup::get_hashed_node_value;

const KAFKA_LISTENER_QUERY: &str = r#"
(method_declaration
  (modifiers) @modifiers
  type: (_)? @return_type
  name: (_) @method_name
  parameters: (_) @method_params
) @method
"#;

const KAFKA_TEMPLATE_MARKER: &str = "KafkaTemplate";
const KAFKA_PRODUCER_MARKER: &str = "KafkaProducer";
const KAFKA_CONSUMER_MARKER: &str = "KafkaConsumer";
const PRODUCER_RECORD_MARKER: &str = "ProducerRecord";
const STREAMS_BUILDER_MARKER: &str = "StreamsBuilder";
const KSTREAM_MARKER: &str = "KStream";
const SPRING_KAFKA_PACKAGE_MARKER: &str = "org.springframework.kafka";
const APACHE_KAFKA_PACKAGE_MARKER: &str = "org.apache.kafka";

const KAFKA_FILE_MARKERS: &[&str] = &[
    KAFKA_TEMPLATE_MARKER,
    KAFKA_PRODUCER_MARKER,
    KAFKA_CONSUMER_MARKER,
    PRODUCER_RECORD_MARKER,
    STREAMS_BUILDER_MARKER,
    KSTREAM_MARKER,
    SPRING_KAFKA_PACKAGE_MARKER,
    APACHE_KAFKA_PACKAGE_MARKER,
];

const KAFKA_STREAM_MARKERS: &[&str] = &[STREAMS_BUILDER_MARKER, KSTREAM_MARKER];
const KAFKA_LISTENER_ANNOTATION: &str = "@KafkaListener";
const VALUE_ANNOTATION_PATTERN: &str = r#"@Value\s*\(\s*"(?P<placeholder>\$\{[^"]+\})"\s*\)\s+(?:(?:private|public|protected|final)\s+)*(?:[\w<>?,\s]+\s+)?(?P<name>[A-Za-z_][A-Za-z0-9_]*)"#;
const STREAM_OUTPUT_PATTERN: &str = r"\.to\s*\(\s*([A-Za-z_][A-Za-z0-9_\.]*)\s*\)";
const STREAM_OUTPUT_METHOD: &str = "to";
const STREAM_OUTPUT_LABEL: &str = "Kafka Streams .to";
const TOPIC_BUILDER_NAME_PREFIX: &str = "TopicBuilder.name(";

const CAPTURE_MODIFIERS: &str = "modifiers";
const CAPTURE_RETURN_TYPE: &str = "return_type";
const CAPTURE_METHOD_NAME: &str = "method_name";
const CAPTURE_METHOD_PARAMS: &str = "method_params";
const CAPTURE_METHOD: &str = "method";
const CAPTURE_NAME_FIELD: &str = "name";
const CAPTURE_PLACEHOLDER_FIELD: &str = "placeholder";

const METHOD_SEND: &str = "send";
const METHOD_SUBSCRIBE: &str = "subscribe";
const METHOD_RECEIVE: &str = "receive";
const METHOD_STREAM: &str = "stream";
const METHOD_NAME: &str = "name";
const METHOD_NEW_TOPIC: &str = "NewTopic";

const JAVA_ANNOTATION_NODE: &str = "annotation";
const JAVA_MARKER_ANNOTATION_NODE: &str = "marker_annotation";
const KAFKA_LISTENER_TOPICS_KEY: &str = "topics";
const KAFKA_LISTENER_TOPIC_KEY: &str = "topic";
const KAFKA_LISTENER_VALUE_KEY: &str = "value";
const KAFKA_LISTENER_KEYS: &[&str] = &[
    KAFKA_LISTENER_TOPICS_KEY,
    KAFKA_LISTENER_TOPIC_KEY,
    KAFKA_LISTENER_VALUE_KEY,
];
const JAVA_PLACEHOLDER_PREFIX: &str = "${";
const JAVA_PLACEHOLDER_SUFFIX: char = '}';

#[derive(Default)]
pub struct KafkaIdentificationStrategy;

impl KafkaIdentificationStrategy {
    pub fn new() -> Self {
        Self {}
    }

    pub fn identify_from_calls(
        &self,
        calls: &[CallStatement],
        code: &str,
        file_path: &str,
    ) -> Vec<MessageEdge> {
        let placeholders = value_placeholders(code);
        let kafka_file = contains_any_marker(code, KAFKA_FILE_MARKERS);
        let streams_file = contains_any_marker(code, KAFKA_STREAM_MARKERS);

        calls
            .iter()
            .filter_map(|call| {
                self.identify_call(call, file_path, &placeholders, kafka_file, streams_file)
            })
            .collect()
    }

    pub fn identify_from_annotations(
        &self,
        code: &str,
        tree: &Tree,
        file_path: &str,
    ) -> Vec<MessageEdge> {
        let query = kafka_listener_query();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), code.as_bytes());
        let placeholders = value_placeholders(code);
        let mut edges = Vec::new();

        while let Some(m) = matches.next() {
            let mut annotations = Vec::new();
            let mut return_type = String::new();
            let mut method_name = String::new();
            let mut method_params = String::new();
            let mut function_hash = String::new();

            for capture in m.captures {
                let value = code[capture.node.start_byte()..capture.node.end_byte()].to_string();
                match query.capture_names()[capture.index as usize] {
                    CAPTURE_MODIFIERS => {
                        annotations = annotations_from_modifiers(capture.node, code)
                    }
                    CAPTURE_RETURN_TYPE => return_type = value,
                    CAPTURE_METHOD_NAME => method_name = value,
                    CAPTURE_METHOD_PARAMS => method_params = value,
                    CAPTURE_METHOD => function_hash = get_hashed_node_value(capture.node, code),
                    _ => {}
                }
            }

            for annotation in annotations
                .iter()
                .filter(|a| a.starts_with(KAFKA_LISTENER_ANNOTATION))
            {
                for topic in extract_kafka_listener_topics(annotation)
                    .into_iter()
                    .map(|topic| resolve_placeholder_alias(&topic, &placeholders))
                {
                    edges.push(MessageEdge {
                        protocol: CommunicationProtocol::Kafka,
                        role: MessageRole::Consumer,
                        destination_kind: MessageDestinationKind::Topic,
                        destination: topic.clone(),
                        exchange: None,
                        routing_key: None,
                        queue: None,
                        topic: Some(topic),
                        handler: Some(method_name.clone()),
                        function_name: format!(
                            "{} {}{}",
                            return_type,
                            method_name,
                            statix::strings::normalize_whitespace(&method_params)
                        )
                        .trim()
                        .to_string(),
                        function_hash: function_hash.clone(),
                        call_arguments: Vec::new(),
                        file_path: file_path.to_string(),
                    });
                }
            }
        }

        edges
    }

    /// Tree-sitter represents a terminal invocation in some fluent Kafka
    /// Streams chains without surfacing it through the generic call extractor.
    /// Recover those `.to(topic)` producer edges directly, while avoiding a
    /// duplicate when the generic extractor did capture the invocation.
    pub fn identify_stream_chain_outputs(
        &self,
        calls: &[CallStatement],
        code: &str,
        file_path: &str,
    ) -> Vec<MessageEdge> {
        if !contains_any_marker(code, KAFKA_STREAM_MARKERS) {
            return Vec::new();
        }
        static TO_RE: OnceLock<Regex> = OnceLock::new();
        let regex = TO_RE.get_or_init(|| {
            Regex::new(STREAM_OUTPUT_PATTERN).expect("valid Kafka Streams output regex")
        });
        let placeholders = value_placeholders(code);
        regex
            .captures_iter(code)
            .filter_map(|captures| captures.get(1).map(|topic| topic.as_str().to_string()))
            .filter(|topic| {
                !calls.iter().any(|call| {
                    method_name(&call.function_name) == STREAM_OUTPUT_METHOD
                        && call
                            .arguments
                            .first()
                            .is_some_and(|arg| clean_java_value(&arg.value) == *topic)
                })
            })
            .map(|topic| {
                let topic = resolve_placeholder_alias(&topic, &placeholders);
                MessageEdge {
                    protocol: CommunicationProtocol::Kafka,
                    role: MessageRole::Producer,
                    destination_kind: MessageDestinationKind::Topic,
                    destination: topic.clone(),
                    exchange: None,
                    routing_key: None,
                    queue: None,
                    topic: Some(topic),
                    handler: None,
                    function_name: STREAM_OUTPUT_LABEL.to_string(),
                    function_hash: String::new(),
                    call_arguments: Vec::new(),
                    file_path: file_path.to_string(),
                }
            })
            .collect()
    }

    fn identify_call(
        &self,
        call: &CallStatement,
        file_path: &str,
        placeholders: &HashMap<String, String>,
        kafka_file: bool,
        streams_file: bool,
    ) -> Option<MessageEdge> {
        let method = method_name(&call.function_name);
        match method.as_str() {
            METHOD_SEND if kafka_file => self.identify_send(call, file_path, placeholders),
            PRODUCER_RECORD_MARKER if kafka_file => {
                self.identify_producer_record(call, file_path, placeholders)
            }
            METHOD_SUBSCRIBE if kafka_file => {
                self.identify_subscribe(call, file_path, placeholders)
            }
            METHOD_RECEIVE if kafka_file => self.identify_receive(call, file_path, placeholders),
            METHOD_STREAM if streams_file => {
                self.identify_stream_consumer(call, file_path, placeholders)
            }
            STREAM_OUTPUT_METHOD if streams_file => {
                self.identify_stream_producer(call, file_path, placeholders)
            }
            METHOD_NAME
                if code_looks_like_topic_builder_target(&call.function_name) || kafka_file =>
            {
                self.identify_topic_builder(call, file_path, placeholders)
            }
            METHOD_NEW_TOPIC if kafka_file => {
                self.identify_new_topic(call, file_path, placeholders)
            }
            _ => None,
        }
    }

    fn identify_send(
        &self,
        call: &CallStatement,
        file_path: &str,
        placeholders: &HashMap<String, String>,
    ) -> Option<MessageEdge> {
        let topic = resolve_placeholder_alias(
            &clean_java_value(&call.arguments.first()?.value),
            placeholders,
        );
        Some(edge(
            MessageRole::Producer,
            topic.clone(),
            Some(topic),
            None,
            call,
            file_path,
        ))
    }

    fn identify_producer_record(
        &self,
        call: &CallStatement,
        file_path: &str,
        placeholders: &HashMap<String, String>,
    ) -> Option<MessageEdge> {
        let topic = resolve_placeholder_alias(
            &clean_java_value(&call.arguments.first()?.value),
            placeholders,
        );
        Some(edge(
            MessageRole::Producer,
            topic.clone(),
            Some(topic),
            None,
            call,
            file_path,
        ))
    }

    fn identify_subscribe(
        &self,
        call: &CallStatement,
        file_path: &str,
        placeholders: &HashMap<String, String>,
    ) -> Option<MessageEdge> {
        let raw = clean_java_value(&call.arguments.first()?.value);
        let topics = split_topics_argument(&raw);
        let topic = resolve_placeholder_alias(topics.first().unwrap_or(&raw), placeholders);
        Some(edge(
            MessageRole::Consumer,
            topic.clone(),
            Some(topic),
            None,
            call,
            file_path,
        ))
    }

    fn identify_receive(
        &self,
        call: &CallStatement,
        file_path: &str,
        placeholders: &HashMap<String, String>,
    ) -> Option<MessageEdge> {
        let topic = resolve_placeholder_alias(
            &clean_java_value(&call.arguments.first()?.value),
            placeholders,
        );
        Some(edge(
            MessageRole::Consumer,
            topic.clone(),
            Some(topic),
            None,
            call,
            file_path,
        ))
    }

    fn identify_stream_consumer(
        &self,
        call: &CallStatement,
        file_path: &str,
        placeholders: &HashMap<String, String>,
    ) -> Option<MessageEdge> {
        let topic = resolve_placeholder_alias(
            &clean_java_value(&call.arguments.first()?.value),
            placeholders,
        );
        Some(edge(
            MessageRole::Consumer,
            topic.clone(),
            Some(topic),
            None,
            call,
            file_path,
        ))
    }

    fn identify_stream_producer(
        &self,
        call: &CallStatement,
        file_path: &str,
        placeholders: &HashMap<String, String>,
    ) -> Option<MessageEdge> {
        let topic = resolve_placeholder_alias(
            &clean_java_value(&call.arguments.first()?.value),
            placeholders,
        );
        Some(edge(
            MessageRole::Producer,
            topic.clone(),
            Some(topic),
            None,
            call,
            file_path,
        ))
    }

    fn identify_topic_builder(
        &self,
        call: &CallStatement,
        file_path: &str,
        placeholders: &HashMap<String, String>,
    ) -> Option<MessageEdge> {
        let topic = resolve_placeholder_alias(
            &clean_java_value(&call.arguments.first()?.value),
            placeholders,
        );
        Some(edge(
            MessageRole::TopicDeclaration,
            topic.clone(),
            Some(topic),
            None,
            call,
            file_path,
        ))
    }

    fn identify_new_topic(
        &self,
        call: &CallStatement,
        file_path: &str,
        placeholders: &HashMap<String, String>,
    ) -> Option<MessageEdge> {
        let topic = resolve_placeholder_alias(
            &clean_java_value(&call.arguments.first()?.value),
            placeholders,
        );
        Some(edge(
            MessageRole::TopicDeclaration,
            topic.clone(),
            Some(topic),
            None,
            call,
            file_path,
        ))
    }
}

fn contains_any_marker(code: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| code.contains(marker))
}

fn kafka_listener_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_java::LANGUAGE.into(), KAFKA_LISTENER_QUERY)
            .expect("Failed to compile Java KafkaListener query")
    })
}

fn edge(
    role: MessageRole,
    destination: String,
    topic: Option<String>,
    handler: Option<String>,
    call: &CallStatement,
    file_path: &str,
) -> MessageEdge {
    MessageEdge {
        protocol: CommunicationProtocol::Kafka,
        role,
        destination_kind: MessageDestinationKind::Topic,
        destination,
        exchange: None,
        routing_key: None,
        queue: None,
        topic,
        handler,
        function_name: call.enclosing_function_name.clone().unwrap_or_default(),
        function_hash: call.enclosing_function_hash.clone().unwrap_or_default(),
        call_arguments: call.arguments.clone(),
        file_path: file_path.to_string(),
    }
}

fn method_name(function_name: &str) -> String {
    let without_args = function_name.split('(').next().unwrap_or(function_name);
    without_args
        .rsplit('.')
        .next()
        .unwrap_or(without_args)
        .to_string()
}

fn annotations_from_modifiers(modifiers_node: Node, code: &str) -> Vec<String> {
    let mut annotations = Vec::new();
    let mut cursor = modifiers_node.walk();
    for child in modifiers_node.children(&mut cursor) {
        if child.kind() == JAVA_ANNOTATION_NODE || child.kind() == JAVA_MARKER_ANNOTATION_NODE {
            annotations.push(code[child.start_byte()..child.end_byte()].to_string());
        }
    }
    annotations
}

fn extract_kafka_listener_topics(annotation: &str) -> Vec<String> {
    let Some(start) = annotation.find('(') else {
        return Vec::new();
    };
    let Some(end) = annotation.rfind(')') else {
        return Vec::new();
    };
    let inside = annotation[start + 1..end].trim();
    if inside.is_empty() {
        return Vec::new();
    }

    extract_named_annotation_values(inside, KAFKA_LISTENER_KEYS)
        .into_iter()
        .map(|v| clean_java_value(&v))
        .filter(|v| !v.is_empty())
        .collect()
}

fn extract_named_annotation_values(inside: &str, keys: &[&str]) -> Vec<String> {
    let parts = statix::strings::split_at_top_level(inside, &[','], &[('(', ')'), ('{', '}')]);
    for part in &parts {
        if let Some((name, value)) = part.split_once('=')
            && keys.iter().any(|key| *key == name.trim())
        {
            return split_annotation_values(value);
        }
    }

    if parts.len() == 1 && !inside.contains('=') {
        return split_annotation_values(inside);
    }

    Vec::new()
}

fn split_annotation_values(value: &str) -> Vec<String> {
    let value = value.trim();
    let value = statix::strings::strip_outer_delimiters(value, '{', '}');
    statix::strings::split_at_top_level(value, &[','], &[('(', ')'), ('{', '}')])
}

fn split_topics_argument(raw: &str) -> Vec<String> {
    let Some(start) = raw.find('(') else {
        return vec![raw.to_string()];
    };
    let Some(end) = raw.rfind(')') else {
        return vec![raw.to_string()];
    };
    statix::strings::split_at_top_level(&raw[start + 1..end], &[','], &[('(', ')'), ('{', '}')])
        .into_iter()
        .map(|topic| clean_java_value(&topic))
        .filter(|topic| !topic.is_empty())
        .collect()
}

fn clean_java_value(raw: &str) -> String {
    let trimmed = raw.trim();
    trimmed
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn value_placeholders(code: &str) -> HashMap<String, String> {
    static VALUE_RE: OnceLock<Regex> = OnceLock::new();
    let value_re =
        VALUE_RE.get_or_init(|| Regex::new(VALUE_ANNOTATION_PATTERN).expect("valid @Value regex"));

    value_re
        .captures_iter(code)
        .filter_map(|captures| {
            let name = captures.name(CAPTURE_NAME_FIELD)?.as_str().to_string();
            let placeholder = captures
                .name(CAPTURE_PLACEHOLDER_FIELD)?
                .as_str()
                .to_string();
            Some((name, placeholder_key(&placeholder)))
        })
        .collect()
}

fn resolve_placeholder_alias(raw: &str, placeholders: &HashMap<String, String>) -> String {
    let cleaned = clean_java_value(raw);
    if let Some(value) = placeholders.get(&cleaned) {
        return value.clone();
    }
    placeholder_key(&cleaned)
}

fn placeholder_key(value: &str) -> String {
    let trimmed = clean_java_value(value);
    if trimmed.starts_with(JAVA_PLACEHOLDER_PREFIX) && trimmed.ends_with(JAVA_PLACEHOLDER_SUFFIX) {
        trimmed[JAVA_PLACEHOLDER_PREFIX.len()..trimmed.len() - 1].to_string()
    } else {
        trimmed
    }
}

fn code_looks_like_topic_builder_target(function_name: &str) -> bool {
    function_name.starts_with(TOPIC_BUILDER_NAME_PREFIX)
}
