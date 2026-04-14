use models::{ConfigurationData, configuration::ServiceDescription};
use std::path::Path;
use synthesizer::{
    connectors::dto::Constant, direct_cm_build, direct_imcg_build, direct_sdg_build,
};

use super::helpers::{extract_from_dirs, fixture_base};

#[tokio::test]
async fn java_two_services_full_pipeline() {
    let base = fixture_base();
    let order_dir = format!("{}/java_order_service", base);
    let billing_dir = format!("{}/java_billing_service", base);

    let aggregate = extract_from_dirs(&[Path::new(&order_dir), Path::new(&billing_dir)]);

    // Extraction assertions
    assert!(
        aggregate.endpoints.len() >= 4,
        "OrderController should produce 4 endpoints, got {}",
        aggregate.endpoints.len()
    );
    assert!(
        aggregate.restcalls.len() >= 2,
        "OrderClient should produce 2 REST calls, got {}",
        aggregate.restcalls.len()
    );
    assert!(
        aggregate.entities.len() >= 3,
        "Should have Order, OrderItem, Invoice, got {}",
        aggregate.entities.len()
    );

    // Symbolic evaluation should have resolved "http://ts-order-service"
    let has_resolved_url = aggregate
        .restcalls
        .iter()
        .any(|r| r.target_uri.contains("http://ts-order-service"));
    assert!(
        has_resolved_url,
        "Symbolic evaluation should resolve ts-order-service URL in REST calls"
    );

    let config = ConfigurationData {
        service_descriptions: vec![
            ServiceDescription {
                name: "order-service".to_string(),
                base_dir_path: order_dir.clone(),
                urls: vec![
                    "http://ts-order-service".to_string(),
                    "http://localhost:8080".to_string(),
                ],
            },
            ServiceDescription {
                name: "billing-service".to_string(),
                base_dir_path: billing_dir.clone(),
                urls: vec!["http://billing-service:8081".to_string()],
            },
        ],
    };
    let constants: Vec<Constant> = vec![];

    let cm = direct_cm_build(&aggregate, &config);
    let sdg = direct_sdg_build(&aggregate, &config, &constants);
    let imcg = direct_imcg_build(&aggregate, &config, &sdg);

    // SDG assertions
    assert_eq!(
        sdg.services.len(),
        2,
        "SDG should have 2 services: order-service and billing-service"
    );
    let order_svc = sdg
        .services
        .iter()
        .find(|s| s.name == "order-service")
        .expect("order-service must be present in SDG");
    assert_eq!(
        order_svc.endpoints.len(),
        4,
        "order-service should expose 4 endpoints"
    );
    assert!(
        sdg.connections.len() >= 1,
        "billing-service should connect to order-service via REST"
    );
    let billing_to_order = sdg
        .connections
        .iter()
        .find(|c| c.source_id == "billing-service" && c.target_id == "order-service");
    assert!(
        billing_to_order.is_some(),
        "Expected a connection from billing-service -> order-service, connections: {:?}",
        sdg.connections
            .iter()
            .map(|c| format!("{}->{}", c.source_id, c.target_id))
            .collect::<Vec<_>>()
    );
    assert!(
        billing_to_order.unwrap().requests.len() >= 1,
        "Connection should have at least 1 matched endpoint-restcall pair"
    );

    // CM assertions
    assert!(
        cm.entities.len() >= 3,
        "CM should contain at least 3 assigned entities"
    );
    let order_service_entities: Vec<_> = cm
        .entities
        .iter()
        .filter(|e| e.service_name == "order-service")
        .collect();
    let billing_service_entities: Vec<_> = cm
        .entities
        .iter()
        .filter(|e| e.service_name == "billing-service")
        .collect();
    assert!(
        order_service_entities.len() >= 2,
        "Order and OrderItem should be in order-service"
    );
    assert!(
        billing_service_entities.len() >= 1,
        "Invoice should be in billing-service"
    );

    // IMCG assertions
    assert!(
        !imcg.callables.is_empty(),
        "IMCG should contain callables from both services"
    );
    assert!(
        !imcg.calls.is_empty(),
        "IMCG should contain calls (intra- and/or inter-service)"
    );
}
