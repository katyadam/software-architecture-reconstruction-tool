use std::path::Path;

use extractor_runtime::pipeline::build_project_ir;
use models::{ConfigurationData, Entity, configuration::ServiceDescription};
use synthesizer::{ContextMapBuilder, ContextMapBuilderImpl};

use super::helpers::{collect_file_records, fixture_base};

fn build_config(service_dirs: &[(&str, &str)]) -> ConfigurationData {
    ConfigurationData {
        service_descriptions: service_dirs
            .iter()
            .map(|(name, dir)| ServiceDescription {
                name: name.to_string(),
                base_dir_path: dir.to_string(),
                urls: vec![],
            })
            .collect(),
    }
}

fn collect_entities(project_ir: &models::ir::project::ProjectIR) -> Vec<Entity> {
    project_ir
        .files
        .iter()
        .flat_map(|f| f.entities.clone())
        .collect()
}

/// Java: Invoice (billing-service) has an explicit import of Order (order-service).
/// Pass 2 resolves Invoice.order.datatype_signature to the Order FQN, which the
/// Context Map builder then turns into a cross-service dependency.
///
/// Known limitation: Order.items (List<OrderItem>) is NOT resolved because Java
/// same-package references require no import statement — the import graph only
/// captures explicit imports.
#[test]
fn java_context_map_via_multipass_pipeline() {
    let base = fixture_base();
    let order_dir = format!("{}/java_order_service", base);
    let billing_dir = format!("{}/java_billing_service", base);

    let file_records = collect_file_records(&[Path::new(&order_dir), Path::new(&billing_dir)]);

    let project_ir = build_project_ir(file_records);

    // Pass 2 assertion: Invoice.order should be resolved to the Order FQN.
    let invoice_entity = project_ir
        .files
        .iter()
        .flat_map(|f| &f.entities)
        .find(|e| e.name == "Invoice")
        .expect("Invoice entity not found in ProjectIR");

    let order_field = invoice_entity
        .fields
        .iter()
        .find(|f| f.name == "order")
        .expect("Invoice.order field not found");

    assert_eq!(
        order_field.datatype_signature.as_deref(),
        Some("java_order_service.model.Order"),
        "Invoice.order should resolve to java_order_service.model.Order via cross-file import graph"
    );

    // Context Map assertions.
    let entities = collect_entities(&project_ir);
    let config = build_config(&[
        ("order-service", &order_dir),
        ("billing-service", &billing_dir),
    ]);

    let cm = ContextMapBuilderImpl::new()
        .build(&entities, &config.service_descriptions)
        .expect("Context Map build failed");

    let invoice_assigned = cm
        .entities
        .iter()
        .find(|e| e.entity.name == "Invoice")
        .expect("Invoice must be in Context Map");
    assert_eq!(invoice_assigned.service_name, "billing-service");

    let order_assigned = cm
        .entities
        .iter()
        .find(|e| e.entity.name == "Order")
        .expect("Order must be in Context Map");
    assert_eq!(order_assigned.service_name, "order-service");

    // There must be a dependency from Invoice -> Order crossing service boundaries.
    let cross_service_dep = cm.dependencies.iter().find(|d| {
        d.source_id == invoice_assigned.entity.signature
            && d.target_id == order_assigned.entity.signature
    });
    assert!(
        cross_service_dep.is_some(),
        "Expected a CM dependency Invoice -> Order across billing-service and order-service, \
         dependencies found: {:?}",
        cm.dependencies
            .iter()
            .map(|d| format!(
                "{} -> {} ({} -> {})",
                d.source_id, d.target_id, d.source_ms, d.target_ms
            ))
            .collect::<Vec<_>>()
    );

    let dep = cross_service_dep.unwrap();
    assert_eq!(dep.source_ms, "billing-service");
    assert_eq!(dep.target_ms, "order-service");
}

/// Python: Invoice (invoice-service) imports Order from python_order_service.models.
/// Pass 2 resolves Invoice.order.datatype_signature to the Order entity signature,
/// producing a cross-service dependency in the Context Map.
#[test]
fn python_context_map_via_multipass_pipeline() {
    let base = fixture_base();
    let order_dir = format!("{}/python_order_service", base);
    let invoice_dir = format!("{}/python_invoice_service", base);

    let file_records = collect_file_records(&[Path::new(&order_dir), Path::new(&invoice_dir)]);

    let project_ir = build_project_ir(file_records);

    // Pass 2 assertion: Invoice.order should be resolved to the Order entity signature.
    let invoice_entity = project_ir
        .files
        .iter()
        .flat_map(|f| &f.entities)
        .find(|e| e.name == "Invoice")
        .expect("Invoice entity not found in ProjectIR");

    let order_field = invoice_entity
        .fields
        .iter()
        .find(|f| f.name == "order")
        .expect("Invoice.order field not found");

    let order_sig = order_field
        .datatype_signature
        .as_deref()
        .expect("Invoice.order datatype_signature should be resolved via cross-file import graph");

    assert!(
        order_sig.contains("python_order_service") && order_sig.ends_with("/Order"),
        "Invoice.order signature should reference python_order_service/.../Order, got: {order_sig}"
    );

    // Context Map assertions.
    let entities = collect_entities(&project_ir);
    let config = build_config(&[
        ("order-service", &order_dir),
        ("invoice-service", &invoice_dir),
    ]);

    let cm = ContextMapBuilderImpl::new()
        .build(&entities, &config.service_descriptions)
        .expect("Context Map build failed");

    let invoice_assigned = cm
        .entities
        .iter()
        .find(|e| e.entity.name == "Invoice")
        .expect("Invoice must be in Context Map");
    assert_eq!(invoice_assigned.service_name, "invoice-service");

    let order_assigned = cm
        .entities
        .iter()
        .find(|e| e.entity.name == "Order")
        .expect("Order must be in Context Map");
    assert_eq!(order_assigned.service_name, "order-service");

    let cross_service_dep = cm.dependencies.iter().find(|d| {
        d.source_id == invoice_assigned.entity.signature
            && d.target_id == order_assigned.entity.signature
    });
    assert!(
        cross_service_dep.is_some(),
        "Expected a CM dependency Invoice -> Order across invoice-service and order-service, \
         dependencies found: {:?}",
        cm.dependencies
            .iter()
            .map(|d| format!(
                "{} -> {} ({} -> {})",
                d.source_id, d.target_id, d.source_ms, d.target_ms
            ))
            .collect::<Vec<_>>()
    );

    let dep = cross_service_dep.unwrap();
    assert_eq!(dep.source_ms, "invoice-service");
    assert_eq!(dep.target_ms, "order-service");
}
