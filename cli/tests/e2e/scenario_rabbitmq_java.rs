use models::{ConfigurationData, MessageRole, configuration::ServiceDescription};
use std::path::Path;
use synthesizer::{connectors::dto::Constant, direct_sdg_build};

use super::helpers::{extract_from_dirs, fixture_base};

#[tokio::test]
async fn java_rabbitmq_pipeline_links_services_by_queue() {
    let base = fixture_base();
    let producer_dir = format!("{}/java_rabbitmq_producer", base);
    let consumer_dir = format!("{}/java_rabbitmq_consumer", base);

    let aggregate = extract_from_dirs(&[Path::new(&producer_dir), Path::new(&consumer_dir)]);

    assert!(
        aggregate.message_edges.iter().any(|edge| {
            edge.role == MessageRole::Producer && edge.queue.as_deref() == Some("orders.created")
        }),
        "expected resolved Java RabbitMQ producer edge, got: {:?}",
        aggregate.message_edges
    );
    assert!(
        aggregate.message_edges.iter().any(|edge| {
            edge.role == MessageRole::Consumer && edge.queue.as_deref() == Some("orders.created")
        }),
        "expected resolved Java RabbitMQ consumer edge, got: {:?}",
        aggregate.message_edges
    );

    let config = ConfigurationData {
        service_descriptions: vec![
            ServiceDescription {
                name: "order-service".to_string(),
                base_dir_path: producer_dir,
                urls: vec![],
            },
            ServiceDescription {
                name: "billing-service".to_string(),
                base_dir_path: consumer_dir,
                urls: vec![],
            },
        ],
    };
    let constants: Vec<Constant> = vec![];
    let sdg = direct_sdg_build(&aggregate, &config, &constants);

    assert!(
        sdg.message_connections.iter().any(|connection| {
            connection.source_id == "order-service"
                && connection.target_id == "billing-service"
                && connection.messages.len() == 1
        }),
        "expected Java RabbitMQ order-service -> billing-service connection, got: {:?}",
        sdg.message_connections
    );
}
