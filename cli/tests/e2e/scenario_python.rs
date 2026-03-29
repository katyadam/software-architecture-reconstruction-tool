use models::{ConfigurationData, configuration::ServiceDescription};
use std::path::Path;
use synthesizer::{
    connectors::dto::Constant, direct_cm_build, direct_imcg_build, direct_sdg_build,
};

use super::helpers::{extract_from_dirs, fixture_base};

#[tokio::test]
async fn python_two_services_full_pipeline() {
    let base = fixture_base();
    let product_dir = format!("{}/python_product_service", base);
    let cart_dir = format!("{}/python_cart_service", base);

    let aggregate = extract_from_dirs(&[Path::new(&product_dir), Path::new(&cart_dir)]).await;

    // Extraction assertions
    assert!(
        aggregate.endpoints.len() >= 4,
        "Product routes.py should produce 4 endpoints, got {}",
        aggregate.endpoints.len()
    );
    assert!(
        aggregate.restcalls.len() >= 2,
        "Cart client.py should produce 2 REST calls, got {}",
        aggregate.restcalls.len()
    );
    assert!(
        aggregate.entities.len() >= 1,
        "Should extract at least Product entity, got {}",
        aggregate.entities.len()
    );

    // Literal URLs in the Python client should be extracted as-is
    let has_target_url = aggregate
        .restcalls
        .iter()
        .any(|r| r.target_uri.contains("product-service"));
    assert!(
        has_target_url,
        "REST call target URIs should contain product-service host, got: {:?}",
        aggregate
            .restcalls
            .iter()
            .map(|r| &r.target_uri)
            .collect::<Vec<_>>()
    );

    let config = ConfigurationData {
        service_descriptions: vec![
            ServiceDescription {
                name: "product-service".to_string(),
                base_dir_path: product_dir.clone(),
                urls: vec![
                    "http://product-service:8000".to_string(),
                    "http://localhost:8000".to_string(),
                ],
            },
            ServiceDescription {
                name: "cart-service".to_string(),
                base_dir_path: cart_dir.clone(),
                urls: vec!["http://cart-service:8001".to_string()],
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
        "SDG should have 2 services: product-service and cart-service"
    );
    let product_svc = sdg
        .services
        .iter()
        .find(|s| s.name == "product-service")
        .expect("product-service must be present in SDG");
    assert_eq!(
        product_svc.endpoints.len(),
        4,
        "product-service should expose 4 endpoints"
    );
    assert!(
        sdg.connections.len() >= 1,
        "cart-service should connect to product-service via REST"
    );
    let cart_to_product = sdg
        .connections
        .iter()
        .find(|c| c.source_id == "cart-service" && c.target_id == "product-service");
    assert!(
        cart_to_product.is_some(),
        "Expected connection cart-service -> product-service, connections: {:?}",
        sdg.connections
            .iter()
            .map(|c| format!("{}->{}", c.source_id, c.target_id))
            .collect::<Vec<_>>()
    );
    assert!(
        cart_to_product.unwrap().requests.len() >= 1,
        "Connection should have at least 1 matched endpoint-restcall pair"
    );

    // CM assertions
    assert!(
        !cm.entities.is_empty(),
        "CM should contain at least Product entity"
    );
    let product_entities: Vec<_> = cm
        .entities
        .iter()
        .filter(|e| e.service_name == "product-service")
        .collect();
    assert!(
        !product_entities.is_empty(),
        "Product entity should be assigned to product-service"
    );

    // IMCG assertions
    assert!(!imcg.callables.is_empty(), "IMCG should contain callables");
    assert!(!imcg.calls.is_empty(), "IMCG should contain calls");
}
