use extractor_runtime::pipeline;
use models::{ConfigurationData, configuration::ServiceDescription};
use std::path::Path;
use synthesizer::{connectors::dto::Constant, direct_cm_build, direct_sdg_build};

use super::helpers::{extract_from_dirs, fixture_base};

#[test]
fn empty_java_file_produces_no_elements() {
    let result = pipeline::dispatch_syntactic("", "Empty.java").unwrap();
    let is_empty = result
        .map(|record| {
            record.endpoints.is_empty()
                && record.raw_restcalls.is_empty()
                && record.entities.is_empty()
                && record.callables.is_empty()
                && record.call_statements.is_empty()
        })
        .unwrap_or(true);
    assert!(is_empty, "Empty Java file should produce no code elements");
}

#[test]
fn empty_python_file_produces_no_elements() {
    let result = pipeline::dispatch_syntactic("", "empty.py").unwrap();
    let is_empty = result
        .map(|record| {
            record.endpoints.is_empty()
                && record.raw_restcalls.is_empty()
                && record.entities.is_empty()
                && record.callables.is_empty()
                && record.call_statements.is_empty()
        })
        .unwrap_or(true);
    assert!(
        is_empty,
        "Empty Python file should produce no code elements"
    );
}

#[test]
fn unsupported_file_extension_returns_none() {
    let result = pipeline::dispatch_syntactic("fn main() {}", "main.rs").unwrap();
    assert!(result.is_none(), "Unsupported extension should return None");
}

#[test]
fn no_restcalls_produces_empty_sdg_connections() {
    let base = fixture_base();
    let order_dir = format!("{}/java_order_service", base);

    let aggregate = extract_from_dirs(&[Path::new(&order_dir)]);

    assert!(
        !aggregate.endpoints.is_empty(),
        "Should have extracted endpoints"
    );
    assert!(
        aggregate.restcalls.is_empty(),
        "java_order_service has no REST calls"
    );

    let config = ConfigurationData {
        service_descriptions: vec![ServiceDescription {
            name: "order-service".to_string(),
            base_dir_path: order_dir,
            urls: vec!["http://ts-order-service".to_string()],
        }],
    };
    let constants: Vec<Constant> = vec![];
    let sdg = direct_sdg_build(&aggregate, &config, &constants);

    assert_eq!(sdg.services.len(), 1);
    assert!(
        sdg.connections.is_empty(),
        "No REST calls means 0 SDG connections"
    );
}

#[test]
fn cleared_entities_produces_empty_context_map() {
    let base = fixture_base();
    let order_dir = format!("{}/java_order_service", base);
    let billing_dir = format!("{}/java_billing_service", base);

    let mut aggregate = extract_from_dirs(&[Path::new(&order_dir), Path::new(&billing_dir)]);

    aggregate.entities.clear();

    let config = ConfigurationData {
        service_descriptions: vec![
            ServiceDescription {
                name: "order-service".to_string(),
                base_dir_path: order_dir,
                urls: vec!["http://ts-order-service".to_string()],
            },
            ServiceDescription {
                name: "billing-service".to_string(),
                base_dir_path: billing_dir,
                urls: vec!["http://billing-service:8081".to_string()],
            },
        ],
    };

    let cm = direct_cm_build(&aggregate, &config);
    assert!(
        cm.entities.is_empty(),
        "No entities means empty Context Map"
    );
    assert!(
        cm.dependencies.is_empty(),
        "No entities means no CM dependencies"
    );
}

/// Files not matching any ServiceDescription get assigned to the anonymous/default service
/// (name = ""). The SDG builder still matches their restcalls against named-service endpoints
/// because the service names differ ("" != "order-service"). Connections from the anonymous
/// service have an empty source_id.
#[test]
fn unassigned_files_produce_anonymous_service_connections() {
    let base = fixture_base();
    let order_dir = format!("{}/java_order_service", base);
    let billing_dir = format!("{}/java_billing_service", base);

    let aggregate = extract_from_dirs(&[Path::new(&order_dir), Path::new(&billing_dir)]);

    // Only configure order-service — billing files get anonymous service name ""
    let config = ConfigurationData {
        service_descriptions: vec![ServiceDescription {
            name: "order-service".to_string(),
            base_dir_path: order_dir,
            urls: vec!["http://ts-order-service".to_string()],
        }],
    };
    let constants: Vec<Constant> = vec![];
    let sdg = direct_sdg_build(&aggregate, &config, &constants);

    assert_eq!(sdg.services.len(), 1, "Only one named service in config");
    // Billing restcalls (anonymous service) still match order-service endpoints
    // and create connections with an empty source_id
    assert!(
        sdg.connections.iter().all(|c| c.source_id.is_empty()),
        "Connections from unassigned files should have empty source_id, got: {:?}",
        sdg.connections
            .iter()
            .map(|c| format!("{}->{}", c.source_id, c.target_id))
            .collect::<Vec<_>>()
    );
}
