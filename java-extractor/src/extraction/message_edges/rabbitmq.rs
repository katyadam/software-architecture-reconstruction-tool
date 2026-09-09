use std::sync::OnceLock;

use models::{
    CallStatement, CommunicationProtocol, MessageDestinationKind, MessageEdge, MessageRole,
};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator, Tree};

use crate::extraction::enclosing_lookup::get_hashed_node_value;

const RABBIT_LISTENER_QUERY: &str = r#"
(method_declaration
  (modifiers) @modifiers
  type: (_)? @return_type
  name: (_) @method_name
  parameters: (_) @method_params
) @method
"#;

const MESSAGE_HANDLER_QUERY: &str = r#"
(class_declaration
  name: (identifier) @class_name
  interfaces: (super_interfaces
    (type_list
      (_) @interface
    )
  )
) @class
"#;

const RABBIT_LISTENER_ANNOTATION: &str = "@RabbitListener";
const CAPTURE_MODIFIERS: &str = "modifiers";
const CAPTURE_RETURN_TYPE: &str = "return_type";
const CAPTURE_METHOD_NAME: &str = "method_name";
const CAPTURE_METHOD_PARAMS: &str = "method_params";
const CAPTURE_METHOD: &str = "method";
const CAPTURE_CLASS_NAME: &str = "class_name";
const CAPTURE_INTERFACE: &str = "interface";
const CAPTURE_CLASS: &str = "class";
const METHOD_CONVERT_AND_SEND: &str = "convertAndSend";
const METHOD_CONVERT_SEND_AND_RECEIVE: &str = "convertSendAndReceive";
const METHOD_BASIC_PUBLISH: &str = "basicPublish";
const METHOD_BASIC_CONSUME: &str = "basicConsume";
const METHOD_SEND_MESSAGE: &str = "sendMessage";
const METHOD_QUEUE: &str = "Queue";
const JAVA_ANNOTATION_NODE: &str = "annotation";
const JAVA_MARKER_ANNOTATION_NODE: &str = "marker_annotation";
const RABBIT_QUEUE_KEY: &str = "queues";
const RABBIT_VALUE_KEY: &str = "value";
const MESSAGE_HANDLER_PREFIX: &str = "MessageHandler<";
const DESTINATION_SEPARATOR: &str = ":";
const EVENT_CONSTRUCTOR_SUFFIXES: &[&str] = &[
    "Created",
    "Updated",
    "Deleted",
    "Reserved",
    "Cancelled",
    "Rejected",
];

#[derive(Default)]
pub struct RabbitMqIdentificationStrategy;

impl RabbitMqIdentificationStrategy {
    pub fn new() -> Self {
        Self {}
    }

    pub fn identify_from_calls(
        &self,
        calls: &[CallStatement],
        file_path: &str,
    ) -> Vec<MessageEdge> {
        calls
            .iter()
            .filter_map(|call| self.identify_call(call, file_path))
            .collect()
    }

    pub fn identify_from_annotations(
        &self,
        code: &str,
        tree: &Tree,
        file_path: &str,
    ) -> Vec<MessageEdge> {
        let query = rabbit_listener_query();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), code.as_bytes());
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
                .filter(|a| a.starts_with(RABBIT_LISTENER_ANNOTATION))
            {
                for queue in extract_rabbit_listener_queues(annotation) {
                    edges.push(MessageEdge {
                        protocol: CommunicationProtocol::RabbitMq,
                        role: MessageRole::Consumer,
                        destination_kind: MessageDestinationKind::Queue,
                        destination: queue.clone(),
                        exchange: None,
                        routing_key: None,
                        queue: Some(queue),
                        topic: None,
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

    pub fn identify_from_message_handlers(
        &self,
        code: &str,
        tree: &Tree,
        file_path: &str,
    ) -> Vec<MessageEdge> {
        let query = message_handler_query();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), code.as_bytes());
        let mut edges = Vec::new();

        while let Some(m) = matches.next() {
            let mut class_name = String::new();
            let mut class_hash = String::new();
            let mut contracts = Vec::new();

            for capture in m.captures {
                let value = code[capture.node.start_byte()..capture.node.end_byte()].to_string();
                match query.capture_names()[capture.index as usize] {
                    CAPTURE_CLASS_NAME => class_name = value,
                    CAPTURE_INTERFACE => {
                        if let Some(contract) = extract_message_handler_contract(&value) {
                            contracts.push(contract);
                        }
                    }
                    CAPTURE_CLASS => class_hash = get_hashed_node_value(capture.node, code),
                    _ => {}
                }
            }

            for contract in contracts {
                edges.push(MessageEdge {
                    protocol: CommunicationProtocol::RabbitMq,
                    role: MessageRole::Consumer,
                    destination_kind: MessageDestinationKind::Queue,
                    destination: contract.clone(),
                    exchange: None,
                    routing_key: None,
                    queue: Some(contract),
                    topic: None,
                    handler: Some(class_name.clone()),
                    function_name: class_name.clone(),
                    function_hash: class_hash.clone(),
                    call_arguments: Vec::new(),
                    file_path: file_path.to_string(),
                });
            }
        }

        edges
    }

    fn identify_call(&self, call: &CallStatement, file_path: &str) -> Option<MessageEdge> {
        let method = method_name(&call.function_name);
        match method.as_str() {
            METHOD_CONVERT_AND_SEND | METHOD_CONVERT_SEND_AND_RECEIVE => {
                self.identify_template_send(call, file_path)
            }
            METHOD_BASIC_PUBLISH => self.identify_basic_publish(call, file_path),
            METHOD_BASIC_CONSUME => self.identify_basic_consume(call, file_path),
            METHOD_SEND_MESSAGE => self.identify_wrapper_send(call, file_path),
            METHOD_QUEUE => self.identify_queue_declaration(call, file_path),
            _ if looks_like_integration_event_constructor(&method) => {
                Some(self.identify_integration_event_constructor(call, file_path, &method))
            }
            _ => None,
        }
    }

    fn identify_template_send(&self, call: &CallStatement, file_path: &str) -> Option<MessageEdge> {
        match call.arguments.len() {
            0 => None,
            1 | 2 => {
                let queue = clean_java_value(&call.arguments[0].value);
                Some(edge(
                    MessageRole::Producer,
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
            _ => {
                let exchange = clean_java_value(&call.arguments[0].value);
                let routing_key = clean_java_value(&call.arguments[1].value);
                Some(edge(
                    MessageRole::Producer,
                    MessageDestinationKind::ExchangeRoutingKey,
                    format!("{exchange}{DESTINATION_SEPARATOR}{routing_key}"),
                    Some(exchange),
                    Some(routing_key),
                    None,
                    None,
                    call,
                    file_path,
                ))
            }
        }
    }

    fn identify_basic_publish(&self, call: &CallStatement, file_path: &str) -> Option<MessageEdge> {
        let exchange = clean_java_value(&call.arguments.first()?.value);
        let routing_key = clean_java_value(&call.arguments.get(1)?.value);
        let exchange = (!exchange.is_empty()).then_some(exchange);
        let destination_kind = if exchange.is_some() {
            MessageDestinationKind::ExchangeRoutingKey
        } else {
            MessageDestinationKind::Queue
        };
        let destination = match &exchange {
            Some(exchange) => format!("{exchange}{DESTINATION_SEPARATOR}{routing_key}"),
            None => routing_key.clone(),
        };
        let queue = matches!(destination_kind, MessageDestinationKind::Queue)
            .then_some(routing_key.clone());

        Some(edge(
            MessageRole::Producer,
            destination_kind,
            destination,
            exchange,
            Some(routing_key),
            queue,
            None,
            call,
            file_path,
        ))
    }

    fn identify_basic_consume(&self, call: &CallStatement, file_path: &str) -> Option<MessageEdge> {
        let queue = clean_java_value(&call.arguments.first()?.value);
        Some(edge(
            MessageRole::Consumer,
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

    fn identify_wrapper_send(&self, call: &CallStatement, file_path: &str) -> Option<MessageEdge> {
        let queue = clean_java_value(&call.arguments.first()?.value);
        Some(edge(
            MessageRole::Producer,
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

    fn identify_queue_declaration(
        &self,
        call: &CallStatement,
        file_path: &str,
    ) -> Option<MessageEdge> {
        let queue = clean_java_value(&call.arguments.first()?.value);
        Some(edge(
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

    fn identify_integration_event_constructor(
        &self,
        call: &CallStatement,
        file_path: &str,
        contract: &str,
    ) -> MessageEdge {
        edge(
            MessageRole::Producer,
            MessageDestinationKind::Queue,
            contract.to_string(),
            None,
            None,
            Some(contract.to_string()),
            None,
            call,
            file_path,
        )
    }
}

fn rabbit_listener_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_java::LANGUAGE.into(), RABBIT_LISTENER_QUERY)
            .expect("Failed to compile Java RabbitListener query")
    })
}

fn message_handler_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_java::LANGUAGE.into(), MESSAGE_HANDLER_QUERY)
            .expect("Failed to compile Java MessageHandler query")
    })
}

#[allow(clippy::too_many_arguments)]
fn edge(
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
        exchange,
        routing_key,
        queue,
        topic: None,
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

fn extract_rabbit_listener_queues(annotation: &str) -> Vec<String> {
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

    let value = if let Some((name, value)) = inside.split_once('=') {
        let name = name.trim();
        if name != RABBIT_QUEUE_KEY && name != RABBIT_VALUE_KEY {
            return Vec::new();
        }
        value.trim()
    } else {
        inside
    };

    split_annotation_values(value)
        .into_iter()
        .map(|v| clean_java_value(&v))
        .filter(|v| !v.is_empty())
        .collect()
}

fn split_annotation_values(value: &str) -> Vec<String> {
    let value = value.trim();
    let value = statix::strings::strip_outer_delimiters(value, '{', '}');
    statix::strings::split_at_top_level(value, &[','], &[('(', ')'), ('{', '}')])
}

fn clean_java_value(raw: &str) -> String {
    let trimmed = raw.trim();
    trimmed
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn extract_message_handler_contract(interface_text: &str) -> Option<String> {
    let start = interface_text.find(MESSAGE_HANDLER_PREFIX)? + MESSAGE_HANDLER_PREFIX.len();
    let end = interface_text[start..].find('>')? + start;
    Some(interface_text[start..end].trim().to_string())
}

fn looks_like_integration_event_constructor(method: &str) -> bool {
    method
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_uppercase())
        && EVENT_CONSTRUCTOR_SUFFIXES
            .iter()
            .any(|suffix| method.ends_with(suffix))
}
