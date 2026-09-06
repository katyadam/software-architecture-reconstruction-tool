use std::collections::HashMap;

use extractor_runtime::pipeline::{build_project_ir, evaluate};
use go_extractor::extraction::extract_syntactic as go_extract;
use models::{CodeElementsAggregate, ConfigurationData, configuration::ServiceDescription};
use synthesizer::direct_sdg_build;

#[test]
fn links_go_kafka_producer_and_consumer_services() {
    let producer = r#"
package orders

func publish(ctx any, writer any) {
    _ = writer.WriteMessages(ctx, kafka.Message{Topic: "orders.created"})
}
"#;
    let consumer = r#"
package billing

func subscribe() {
    _ = kafka.NewReader(kafka.ReaderConfig{Topic: "orders.created"})
}
"#;

    let project_ir = build_project_ir(vec![
        go_extract(producer, "services/orders/publisher.go").expect("producer should parse"),
        go_extract(consumer, "services/billing/consumer.go").expect("consumer should parse"),
    ]);
    assert_eq!(
        project_ir.files[0].raw_message_edges.len(),
        1,
        "raw edges: {:?}; calls: {:?}",
        project_ir.files[0].raw_message_edges,
        project_ir.files[0].call_statements
    );
    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );
    let elements = CodeElementsAggregate::from(evaluated);
    assert_eq!(
        elements.message_edges.len(),
        2,
        "evaluated edges: {:?}",
        elements.message_edges
    );
    let configuration = messaging_configuration();

    let sdg = direct_sdg_build(&elements, &configuration, &[]);

    assert_eq!(
        sdg.message_connections.len(),
        1,
        "connections: {sdg:?}; evaluated edges: {:?}",
        elements.message_edges
    );
    let connection = &sdg.message_connections[0];
    assert_eq!(connection.source_id, "orders");
    assert_eq!(connection.target_id, "billing");
    assert_eq!(connection.messages.len(), 1);
    assert_eq!(
        connection.messages[0].producer.destination,
        "orders.created"
    );
    assert_eq!(
        connection.messages[0].consumer.destination,
        "orders.created"
    );
}

#[test]
fn links_go_rabbitmq_publisher_and_queue_binding_services() {
    let publisher = r#"
package orders

import amqp "github.com/rabbitmq/amqp091-go"

func publish(channel any) {
    _ = channel.Publish("orders", "created", false, false, amqp.Publishing{})
}
"#;
    let binding = r#"
package billing

import amqp "github.com/rabbitmq/amqp091-go"

func bind(channel any) {
    _ = channel.QueueBind("billing", "created", "orders", false, nil)
}
"#;

    let project_ir = build_project_ir(vec![
        go_extract(publisher, "services/orders/publisher.go").expect("publisher should parse"),
        go_extract(binding, "services/billing/binding.go").expect("binding should parse"),
    ]);
    let elements = CodeElementsAggregate::from(evaluate(
        project_ir,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    ));

    let sdg = direct_sdg_build(&elements, &messaging_configuration(), &[]);

    assert_eq!(sdg.message_connections.len(), 1, "connections: {sdg:?}");
    let connection = &sdg.message_connections[0];
    assert_eq!(connection.source_id, "orders");
    assert_eq!(connection.target_id, "billing");
    assert_eq!(connection.messages.len(), 1);
    assert_eq!(
        connection.messages[0].producer.destination,
        "orders:created"
    );
    assert_eq!(
        connection.messages[0].consumer.destination,
        "orders:created"
    );
}

fn messaging_configuration() -> ConfigurationData {
    ConfigurationData {
        service_descriptions: vec![
            ServiceDescription {
                name: "orders".to_string(),
                base_dir_path: "services/orders".to_string(),
                urls: vec![],
            },
            ServiceDescription {
                name: "billing".to_string(),
                base_dir_path: "services/billing".to_string(),
                urls: vec![],
            },
        ],
    }
}
