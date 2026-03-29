use models::{ConfigurationData, configuration::ServiceDescription};
use std::path::Path;
use synthesizer::{
    connectors::dto::Constant, direct_cm_build, direct_imcg_build, direct_sdg_build,
};

use super::helpers::{extract_from_dirs, fixture_base};

/// Java Spring endpoints + Python httpx client calling the same Java service.
#[tokio::test]
async fn mixed_java_python_full_pipeline() {
    let base = fixture_base();
    let java_order_dir = format!("{}/java_order_service", base);
    let python_client_dir = format!("{}/python_client_service", base);

    let aggregate =
        extract_from_dirs(&[Path::new(&java_order_dir), Path::new(&python_client_dir)]).await;

    // Java side: endpoints from Spring controller
    assert!(
        aggregate.endpoints.len() >= 4,
        "Java OrderController should produce 4 endpoints, got {}",
        aggregate.endpoints.len()
    );

    // Python side: REST calls from httpx client
    assert!(
        aggregate.restcalls.len() >= 2,
        "Python order_client.py should produce 2 REST calls, got {}",
        aggregate.restcalls.len()
    );

    // Literal URLs in the Python client should contain the target host
    let has_target_url = aggregate
        .restcalls
        .iter()
        .any(|r| r.target_uri.contains("ts-order-service"));
    assert!(
        has_target_url,
        "Python REST call target URIs should contain ts-order-service, got: {:?}",
        aggregate
            .restcalls
            .iter()
            .map(|r| &r.target_uri)
            .collect::<Vec<_>>()
    );

    let config = ConfigurationData {
        service_descriptions: vec![
            ServiceDescription {
                name: "order-service".to_string(),
                base_dir_path: java_order_dir.clone(),
                urls: vec![
                    "http://ts-order-service".to_string(),
                    "http://localhost:8080".to_string(),
                ],
            },
            ServiceDescription {
                name: "python-client-service".to_string(),
                base_dir_path: python_client_dir.clone(),
                urls: vec!["http://python-client:8001".to_string()],
            },
        ],
    };
    let constants: Vec<Constant> = vec![];

    let cm = direct_cm_build(&aggregate, &config);
    let sdg = direct_sdg_build(&aggregate, &config, &constants);
    let imcg = direct_imcg_build(&aggregate, &config, &sdg);

    // SDG: cross-language connection should be detected
    assert_eq!(sdg.services.len(), 2, "SDG should have 2 services");
    assert!(
        sdg.connections.len() >= 1,
        "python-client-service should connect to order-service"
    );
    let py_to_java = sdg
        .connections
        .iter()
        .find(|c| c.source_id == "python-client-service" && c.target_id == "order-service");
    assert!(
        py_to_java.is_some(),
        "Expected cross-language connection python-client-service -> order-service, got: {:?}",
        sdg.connections
            .iter()
            .map(|c| format!("{}->{}", c.source_id, c.target_id))
            .collect::<Vec<_>>()
    );
    assert!(
        py_to_java.unwrap().requests.len() >= 1,
        "Cross-language connection should have matched endpoint-restcall pairs"
    );

    // Verify HTTP methods match across language boundary
    for req in &py_to_java.unwrap().requests {
        assert_eq!(
            req.endpoint.http_method, req.restcall.http_method,
            "HTTP method must match between Java endpoint and Python restcall"
        );
    }

    // IMCG populated
    assert!(!imcg.callables.is_empty());

    // CM: Java service has entities; Python client has none (no class definitions)
    let order_entities: Vec<_> = cm
        .entities
        .iter()
        .filter(|e| e.service_name == "order-service")
        .collect();
    assert!(
        order_entities.len() >= 2,
        "Order and OrderItem should be assigned to order-service"
    );
}
