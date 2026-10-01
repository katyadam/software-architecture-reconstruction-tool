use models::{
    CommunicationProtocol, ConfigurationData, MessageRole, configuration::ServiceDescription,
};
use std::path::Path;
use synthesizer::{connectors::dto::Constant, direct_sdg_build};

use super::helpers::{extract_from_dirs, fixture_base};

#[tokio::test]
async fn java_kafka_pipeline_links_services_by_topic() {
    let base = fixture_base();
    let producer_dir = format!("{}/java_kafka_producer", base);
    let payment_dir = format!("{}/java_kafka_payment_consumer", base);
    let stock_dir = format!("{}/java_kafka_stock_consumer", base);

    let aggregate = extract_from_dirs(&[
        Path::new(&producer_dir),
        Path::new(&payment_dir),
        Path::new(&stock_dir),
    ]);

    assert!(
        aggregate.message_edges.iter().any(|edge| {
            edge.protocol == CommunicationProtocol::Kafka
                && edge.role == MessageRole::Producer
                && edge.topic.as_deref() == Some("orders")
        }),
        "expected resolved Java Kafka producer edge, got: {:?}",
        aggregate.message_edges
    );
    assert!(
        aggregate.message_edges.iter().any(|edge| {
            edge.protocol == CommunicationProtocol::Kafka
                && edge.role == MessageRole::Consumer
                && edge.topic.as_deref() == Some("orders")
        }),
        "expected resolved Java Kafka consumer edge, got: {:?}",
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
                name: "payment-service".to_string(),
                base_dir_path: payment_dir,
                urls: vec![],
            },
            ServiceDescription {
                name: "stock-service".to_string(),
                base_dir_path: stock_dir,
                urls: vec![],
            },
        ],
    };
    let constants: Vec<Constant> = vec![];
    let sdg = direct_sdg_build(&aggregate, &config, &constants);

    assert!(
        sdg.message_connections.iter().any(|connection| {
            connection.source_id == "order-service"
                && connection.target_id == "payment-service"
                && connection.messages.len() == 1
        }),
        "expected Java Kafka order-service -> payment-service connection, got: {:?}",
        sdg.message_connections
    );
    assert!(
        sdg.message_connections.iter().any(|connection| {
            connection.source_id == "order-service"
                && connection.target_id == "stock-service"
                && connection.messages.len() == 1
        }),
        "expected Java Kafka order-service -> stock-service connection, got: {:?}",
        sdg.message_connections
    );
}
