use extractor_runtime::pipeline::{build_project_ir, dispatch_syntactic, evaluate};
use models::{
    CommunicationProtocol, Endpoint, HttpMethod, MessageDestinationKind, MessageEdge, MessageRole,
    RestCall,
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
    let edges = extract_all_edges(project_dir)?;
    let mut links = match_cross_module_links(&edges.message_edges, project_dir, module_roots);
    links.extend(match_rest_links(
        &edges.restcalls,
        &edges.endpoints,
        project_dir,
        module_roots,
    ));
    links.dedup_by(|a, b| {
        a.producer_file == b.producer_file
            && a.consumer_file == b.consumer_file
            && a.destination == b.destination
    });
    eprintln!(
        "[tia] messaging extraction: {} msg-edges + {} rest-calls → {} cross-module links in {}ms",
        edges.message_edges.len(),
        edges.restcalls.len(),
        links.len(),
        started.elapsed().as_millis()
    );
    Ok(links)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

struct AllEdges {
    message_edges: Vec<MessageEdge>,
    restcalls: Vec<RestCall>,
    endpoints: Vec<Endpoint>,
}

fn extract_all_edges(project_dir: &Path) -> anyhow::Result<AllEdges> {
    let mut file_records = Vec::new();
    collect_source_files(project_dir, project_dir, &mut file_records);
    if file_records.is_empty() {
        return Ok(AllEdges {
            message_edges: vec![],
            restcalls: vec![],
            endpoints: vec![],
        });
    }
    let project_ir = build_project_ir(file_records);
    let ir = evaluate(
        project_ir,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );
    Ok(AllEdges {
        message_edges: ir.message_edges,
        restcalls: ir.restcalls,
        endpoints: ir.endpoints,
    })
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

fn match_rest_links(
    restcalls: &[RestCall],
    endpoints: &[Endpoint],
    project_dir: &Path,
    module_roots: &[PathBuf],
) -> Vec<MessagingLink> {
    let mut links: Vec<MessagingLink> = Vec::new();
    for restcall in restcalls {
        let caller_module = module_for_file(&restcall.file_path, project_dir, module_roots);
        for endpoint in endpoints {
            if !http_methods_match(restcall.http_method, endpoint.http_method) {
                continue;
            }
            if !rest_uris_match(&restcall.target_uri, &endpoint.uri) {
                continue;
            }
            let endpoint_module = module_for_file(&endpoint.file_path, project_dir, module_roots);
            if caller_module != endpoint_module {
                links.push(MessagingLink {
                    producer_file: restcall.file_path.clone(),
                    consumer_file: endpoint.file_path.clone(),
                    destination: endpoint.uri.clone(),
                    protocol: CommunicationProtocol::Http,
                });
            }
        }
    }
    links
}

/// Returns true when both methods are the same, or either side is unknown/unset.
fn http_methods_match(caller: HttpMethod, endpoint: HttpMethod) -> bool {
    caller == endpoint
}

/// Returns true when the caller's target URI path matches the endpoint URI (possibly with
/// path-param templates like `{id}`).
fn rest_uris_match(caller_uri: &str, endpoint_uri: &str) -> bool {
    let caller_path = extract_uri_path(caller_uri);
    if normalize_path(caller_path) == normalize_path(endpoint_uri) {
        return true;
    }
    path_template_matches(endpoint_uri, caller_path)
}

/// Strips scheme and authority from a full URL, returning the path component.
/// `"http://svc:8080/api/orders"` → `"/api/orders"`
/// `"/api/orders"` → `"/api/orders"`
fn extract_uri_path(uri: &str) -> &str {
    let after_scheme = uri
        .strip_prefix("https://")
        .or_else(|| uri.strip_prefix("http://"))
        .unwrap_or(uri);
    if let Some(slash) = after_scheme.find('/') {
        &after_scheme[slash..]
    } else {
        "/"
    }
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim_start_matches('/').trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_owned()
    } else {
        format!("/{trimmed}")
    }
}

/// Returns true when `template` path segments match `actual`, treating `{…}` segments as
/// wildcards. Both inputs may optionally start with `/`.
fn path_template_matches(template: &str, actual: &str) -> bool {
    let t_segs: Vec<_> = template.trim_matches('/').split('/').collect();
    let a_segs: Vec<_> = actual.trim_matches('/').split('/').collect();
    if t_segs.len() != a_segs.len() {
        return false;
    }
    t_segs
        .iter()
        .zip(a_segs.iter())
        .all(|(t, a)| (t.starts_with('{') && t.ends_with('}')) || t == a)
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
    use models::{CommunicationProtocol, Endpoint, HttpMethod, MessageDestinationKind, MessageRole, RestCall};

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

    fn make_restcall(target_uri: &str, method: HttpMethod, file_path: &str) -> RestCall {
        RestCall {
            function_name: "fn".into(),
            function_hash: "hash".into(),
            call_arguments: vec![],
            http_method: method,
            target_uri: target_uri.into(),
            file_path: file_path.into(),
        }
    }

    fn make_endpoint(uri: &str, method: HttpMethod, file_path: &str) -> Endpoint {
        Endpoint {
            function_name: "fn".into(),
            function_hash: "hash".into(),
            http_method: method,
            parameters: vec![],
            uri: uri.into(),
            file_path: file_path.into(),
            router_variable: None,
        }
    }

    #[test]
    fn rest_caller_matches_endpoint_in_other_module() {
        let restcalls = vec![make_restcall(
            "http://payment-service:8080/api/payments",
            HttpMethod::POST,
            "order-service/src/main/java/OrderClient.java",
        )];
        let endpoints = vec![make_endpoint(
            "/api/payments",
            HttpMethod::POST,
            "payment-service/src/main/java/PaymentController.java",
        )];
        let project = Path::new("/project");
        let modules = vec![
            PathBuf::from("/project/order-service"),
            PathBuf::from("/project/payment-service"),
        ];
        let links = match_rest_links(&restcalls, &endpoints, project, &modules);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].protocol, CommunicationProtocol::Http);
        assert_eq!(links[0].destination, "/api/payments");
        assert_eq!(
            links[0].producer_file,
            "order-service/src/main/java/OrderClient.java"
        );
    }

    #[test]
    fn rest_path_template_matches() {
        let restcalls = vec![make_restcall(
            "http://svc-b/api/orders/42",
            HttpMethod::GET,
            "svc-a/Client.java",
        )];
        let endpoints = vec![make_endpoint(
            "/api/orders/{id}",
            HttpMethod::GET,
            "svc-b/OrderController.java",
        )];
        let project = Path::new("/p");
        let modules = vec![PathBuf::from("/p/svc-a"), PathBuf::from("/p/svc-b")];
        let links = match_rest_links(&restcalls, &endpoints, project, &modules);
        assert_eq!(links.len(), 1, "template param must match concrete segment");
    }

    #[test]
    fn rest_method_mismatch_not_matched() {
        let restcalls = vec![make_restcall(
            "http://svc-b/api/payments",
            HttpMethod::GET,
            "svc-a/Client.java",
        )];
        let endpoints = vec![make_endpoint(
            "/api/payments",
            HttpMethod::POST,
            "svc-b/Controller.java",
        )];
        let project = Path::new("/p");
        let modules = vec![PathBuf::from("/p/svc-a"), PathBuf::from("/p/svc-b")];
        let links = match_rest_links(&restcalls, &endpoints, project, &modules);
        assert!(links.is_empty(), "method mismatch must not produce a link");
    }

    #[test]
    fn rest_same_module_not_matched() {
        let restcalls = vec![make_restcall(
            "http://svc-a/api/internal",
            HttpMethod::GET,
            "svc-a/InternalClient.java",
        )];
        let endpoints = vec![make_endpoint(
            "/api/internal",
            HttpMethod::GET,
            "svc-a/InternalController.java",
        )];
        let project = Path::new("/p");
        let modules = vec![PathBuf::from("/p/svc-a"), PathBuf::from("/p/svc-b")];
        let links = match_rest_links(&restcalls, &endpoints, project, &modules);
        assert!(links.is_empty(), "same-module REST pair must be filtered out");
    }

    #[test]
    fn uri_path_extraction() {
        assert_eq!(extract_uri_path("http://svc:8080/api/orders"), "/api/orders");
        assert_eq!(extract_uri_path("https://svc/api/v1/items"), "/api/v1/items");
        assert_eq!(extract_uri_path("/api/orders"), "/api/orders");
        assert_eq!(extract_uri_path("http://svc"), "/");
    }

    #[test]
    fn path_template_matching() {
        assert!(path_template_matches("/api/orders/{id}", "/api/orders/42"));
        assert!(path_template_matches("/api/{svc}/items/{id}", "/api/shop/items/7"));
        assert!(!path_template_matches("/api/orders/{id}", "/api/payments/42"));
        assert!(!path_template_matches("/api/orders/{id}", "/api/orders/42/detail"));
    }
}
