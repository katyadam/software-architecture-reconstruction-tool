use extractor_runtime::pipeline::{build_project_ir, dispatch_syntactic, evaluate};
use models::{
    CommunicationProtocol, MessageDestinationKind, MessageEdge, MessageRole,
};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

/// A single cross-module messaging dependency: a producer callable whose file changed
/// has a matching consumer callable in a different Maven module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessagingLink {
    /// Source-relative path of the producer file (matches `ChangedElement.source_path`).
    pub producer_file: String,
    /// Source-relative path of the consumer file.
    pub consumer_file: String,
    /// Resolved topic / queue / exchange name.
    pub destination: String,
    /// Transport protocol.
    pub protocol: CommunicationProtocol,
}

impl MessagingLink {
    pub fn protocol_label(&self) -> &'static str {
        match self.protocol {
            CommunicationProtocol::Kafka => "kafka",
            CommunicationProtocol::RabbitMq => "rabbitmq",
            CommunicationProtocol::Grpc => "grpc",
            CommunicationProtocol::Http => "http",
        }
    }
}

/// Extracts all cross-module messaging links for `project_dir`.
///
/// Runs Pass 1–3 on all source files under `project_dir`, then matches
/// producer edges to consumer edges across different `module_roots`.
/// Returns only pairs where producer and consumer belong to different modules.
///
/// Returns an empty list (not an error) if no source files are found or if
/// extraction fails for a file.
pub fn extract_messaging_links(
    project_dir: &Path,
    module_roots: &[PathBuf],
) -> anyhow::Result<Vec<MessagingLink>> {
    if module_roots.len() < 2 {
        return Ok(vec![]);
    }
    let started = Instant::now();
    let edges = extract_all_message_edges(project_dir)?;
    let links = match_cross_module_links(&edges, project_dir, module_roots);
    eprintln!(
        "[tia] messaging extraction: {} msg-edges → {} cross-module links in {}ms",
        edges.len(),
        links.len(),
        started.elapsed().as_millis()
    );
    Ok(links)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn extract_all_message_edges(project_dir: &Path) -> anyhow::Result<Vec<MessageEdge>> {
    let mut file_records = Vec::new();
    collect_source_files(project_dir, project_dir, &mut file_records);
    if file_records.is_empty() {
        return Ok(vec![]);
    }
    let project_ir = build_project_ir(file_records);
    let ir = evaluate(
        project_ir,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );
    Ok(ir.message_edges)
}

/// Recursively collects all extractable source files, skipping hidden dirs, build
/// output dirs, and other noise.
fn collect_source_files(
    project_dir: &Path,
    dir: &Path,
    out: &mut Vec<models::ir::syntax::FileRecord>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };
        if path.is_dir() {
            if should_skip_dir(&name) {
                continue;
            }
            collect_source_files(project_dir, &path, out);
        } else if is_extractable(&name) {
            let rel = match path.strip_prefix(project_dir) {
                Ok(r) => r.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };
            let text = match fs::read_to_string(&path) {
                Ok(t) => t,
                Err(_) => continue,
            };
            if let Ok(Some(record)) = dispatch_syntactic(&text, &rel) {
                out.push(record);
            }
        }
    }
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        "target" | "build" | "out" | "dist" | "node_modules" | ".git" | "__pycache__"
    ) || name.starts_with('.')
}

fn is_extractable(name: &str) -> bool {
    matches!(
        name.rsplit('.').next().unwrap_or(""),
        "java" | "py" | "go" | "yml" | "yaml" | "properties"
    )
}

fn match_cross_module_links(
    edges: &[MessageEdge],
    project_dir: &Path,
    module_roots: &[PathBuf],
) -> Vec<MessagingLink> {
    let producers: Vec<_> = edges
        .iter()
        .filter(|e| e.role == MessageRole::Producer)
        .collect();
    let consumers: Vec<_> = edges
        .iter()
        .filter(|e| e.role == MessageRole::Consumer)
        .collect();
    let bindings: Vec<_> = edges
        .iter()
        .filter(|e| e.role == MessageRole::Binding)
        .collect();

    let mut links: Vec<MessagingLink> = Vec::new();

    for producer in &producers {
        let prod_module = module_for_file(&producer.file_path, project_dir, module_roots);

        // Direct match: Kafka topic, Queue, gRPC
        for consumer in &consumers {
            if edge_destinations_match(producer, consumer) {
                let cons_module = module_for_file(&consumer.file_path, project_dir, module_roots);
                if prod_module != cons_module {
                    push_link(&mut links, producer, consumer);
                }
            }
        }

        // RabbitMQ topology: producer → binding → consumer
        if producer.protocol == CommunicationProtocol::RabbitMq {
            for binding in &bindings {
                if !rabbitmq_producer_matches_binding(producer, binding) {
                    continue;
                }
                for consumer in &consumers {
                    if consumer.protocol == CommunicationProtocol::RabbitMq
                        && rabbitmq_binding_matches_consumer(binding, consumer)
                    {
                        let cons_module =
                            module_for_file(&consumer.file_path, project_dir, module_roots);
                        if prod_module != cons_module {
                            push_link(&mut links, producer, consumer);
                        }
                    }
                }
            }
        }
    }

    links
}

/// Checks whether a producer edge directly reaches a consumer edge by destination.
fn edge_destinations_match(producer: &MessageEdge, consumer: &MessageEdge) -> bool {
    if producer.protocol != consumer.protocol {
        return false;
    }
    match &producer.destination_kind {
        MessageDestinationKind::Topic => {
            producer.topic.is_some() && producer.topic == consumer.topic
        }
        MessageDestinationKind::Queue => {
            producer.queue.is_some() && producer.queue == consumer.queue
        }
        MessageDestinationKind::ExchangeRoutingKey => {
            producer.exchange.is_some()
                && producer.exchange == consumer.exchange
                && producer.routing_key == consumer.routing_key
        }
        MessageDestinationKind::GrpcService => {
            grpc_service_matches(&producer.destination, &consumer.destination)
        }
        MessageDestinationKind::Unknown => false,
    }
}

fn rabbitmq_producer_matches_binding(producer: &MessageEdge, binding: &MessageEdge) -> bool {
    binding.protocol == CommunicationProtocol::RabbitMq
        && producer.exchange.is_some()
        && producer.exchange == binding.exchange
        && producer.routing_key == binding.routing_key
}

fn rabbitmq_binding_matches_consumer(binding: &MessageEdge, consumer: &MessageEdge) -> bool {
    binding.queue.is_some() && binding.queue == consumer.queue
}

fn grpc_service_matches(a: &str, b: &str) -> bool {
    canonical_grpc_service(a) == canonical_grpc_service(b)
}

fn canonical_grpc_service(destination: &str) -> &str {
    let base = destination.split('/').next().unwrap_or_default();
    base.strip_suffix("Service").unwrap_or(base)
}

fn module_for_file<'a>(
    file_path: &str,
    project_dir: &Path,
    module_roots: &'a [PathBuf],
) -> Option<&'a PathBuf> {
    let abs = project_dir.join(file_path);
    module_roots
        .iter()
        .filter(|m| abs.starts_with(m.as_path()))
        .max_by_key(|m| m.components().count())
}

fn push_link(links: &mut Vec<MessagingLink>, producer: &MessageEdge, consumer: &MessageEdge) {
    links.push(MessagingLink {
        producer_file: producer.file_path.clone(),
        consumer_file: consumer.file_path.clone(),
        destination: producer.destination.clone(),
        protocol: producer.protocol.clone(),
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use models::{CommunicationProtocol, MessageDestinationKind, MessageRole};

    fn make_edge(
        role: MessageRole,
        protocol: CommunicationProtocol,
        destination_kind: MessageDestinationKind,
        destination: &str,
        topic: Option<&str>,
        queue: Option<&str>,
        exchange: Option<&str>,
        routing_key: Option<&str>,
        file_path: &str,
    ) -> MessageEdge {
        MessageEdge {
            role,
            protocol,
            destination_kind,
            destination: destination.into(),
            topic: topic.map(str::to_owned),
            queue: queue.map(str::to_owned),
            exchange: exchange.map(str::to_owned),
            routing_key: routing_key.map(str::to_owned),
            handler: None,
            function_name: "fn".into(),
            function_hash: "hash".into(),
            call_arguments: vec![],
            file_path: file_path.into(),
        }
    }

    #[test]
    fn kafka_cross_module_match() {
        let edges = vec![
            make_edge(
                MessageRole::Producer,
                CommunicationProtocol::Kafka,
                MessageDestinationKind::Topic,
                "order-events",
                Some("order-events"),
                None, None, None,
                "order-service/src/main/java/OrderPublisher.java",
            ),
            make_edge(
                MessageRole::Consumer,
                CommunicationProtocol::Kafka,
                MessageDestinationKind::Topic,
                "order-events",
                Some("order-events"),
                None, None, None,
                "payment-service/src/main/java/PaymentConsumer.java",
            ),
        ];
        let project = Path::new("/project");
        let modules = vec![
            PathBuf::from("/project/order-service"),
            PathBuf::from("/project/payment-service"),
        ];
        let links = match_cross_module_links(&edges, project, &modules);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].destination, "order-events");
        assert_eq!(links[0].producer_file, "order-service/src/main/java/OrderPublisher.java");
        assert_eq!(links[0].consumer_file, "payment-service/src/main/java/PaymentConsumer.java");
    }

    #[test]
    fn same_module_not_matched() {
        let edges = vec![
            make_edge(
                MessageRole::Producer,
                CommunicationProtocol::Kafka,
                MessageDestinationKind::Topic,
                "events",
                Some("events"),
                None, None, None,
                "service-a/src/Producer.java",
            ),
            make_edge(
                MessageRole::Consumer,
                CommunicationProtocol::Kafka,
                MessageDestinationKind::Topic,
                "events",
                Some("events"),
                None, None, None,
                "service-a/src/Consumer.java",
            ),
        ];
        let project = Path::new("/project");
        let modules = vec![
            PathBuf::from("/project/service-a"),
            PathBuf::from("/project/service-b"),
        ];
        let links = match_cross_module_links(&edges, project, &modules);
        assert!(links.is_empty(), "same-module pair must be filtered out");
    }

    #[test]
    fn topic_mismatch_not_matched() {
        let edges = vec![
            make_edge(
                MessageRole::Producer,
                CommunicationProtocol::Kafka,
                MessageDestinationKind::Topic,
                "topic-a",
                Some("topic-a"),
                None, None, None,
                "svc-a/Producer.java",
            ),
            make_edge(
                MessageRole::Consumer,
                CommunicationProtocol::Kafka,
                MessageDestinationKind::Topic,
                "topic-b",
                Some("topic-b"),
                None, None, None,
                "svc-b/Consumer.java",
            ),
        ];
        let project = Path::new("/p");
        let modules = vec![PathBuf::from("/p/svc-a"), PathBuf::from("/p/svc-b")];
        let links = match_cross_module_links(&edges, project, &modules);
        assert!(links.is_empty());
    }
}
