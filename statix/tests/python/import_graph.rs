use std::collections::HashMap;

use models::{
    Callable, Entity, Import, Namespace, ParsedCallable,
    enums::EnumDefinition,
    ir::{ast::CallableAst, language::Language, project::ImportKind, syntax::FileRecord},
};
use statix::import_graph::build_import_graph;

fn make_file_record(file_path: &str) -> FileRecord {
    FileRecord {
        file_path: file_path.to_string(),
        language: Language::Python,
        imports: vec![],
        entities: vec![],
        endpoints: vec![],
        callables: vec![],
        call_statements: vec![],
        assignments: HashMap::new(),
        enums: vec![],
        raw_restcalls: vec![],
        proto_services: vec![],
    }
}

fn make_entity(name: &str, file_path: &str) -> Entity {
    Entity {
        name: name.to_string(),
        superclasses: vec![],
        fields: vec![],
        signature: format!("{}/{}", file_path, name),
        file_path: file_path.to_string(),
    }
}

fn make_callable(name: &str, return_type: &str, file_path: &str) -> ParsedCallable {
    let full_name = format!("{} {}", return_type, name);
    ParsedCallable {
        metadata: Callable {
            name: full_name.clone(),
            signature: format!("{}/{}", file_path, full_name),
            namespace: Namespace::default(),
            parameters: vec![],
            return_type: Some(return_type.to_string()),
            is_async: false,
            is_constructor: false,
            hash: String::new(),
            file_path: file_path.to_string(),
        },
        ast: CallableAst {
            statements: vec![],
            nested: vec![],
        },
    }
}

fn make_import(orig_module: &str, orig_name: &str, codeword: &str) -> Import {
    Import {
        orig_module: orig_module.to_string(),
        orig_name: orig_name.to_string(),
        module_alias: orig_module.to_string(),
        name_alias: orig_name.to_string(),
        codeword: codeword.to_string(),
    }
}

#[test]
fn python_from_import_resolves_entity() {
    let mut file_b = make_file_record("myapp/services.py");
    file_b
        .entities
        .push(make_entity("UserService", "myapp/services.py"));

    let mut file_a = make_file_record("myapp/main.py");
    file_a
        .imports
        .push(make_import("myapp.services", "UserService", "UserService"));

    let graph = build_import_graph(&[file_a, file_b]);

    let ri = graph
        .lookup("myapp/main.py", "UserService")
        .expect("UserService should resolve");
    assert_eq!(ri.source_file, "myapp/services.py");
    assert!(matches!(ri.kind, ImportKind::Entity));
    assert!(ri.fully_qualified_name.contains("UserService"));
}

#[test]
fn python_from_import_resolves_callable() {
    let mut file_b = make_file_record("myapp/utils.py");
    file_b
        .callables
        .push(make_callable("parse_token()", "str", "myapp/utils.py"));

    let mut file_a = make_file_record("myapp/auth.py");
    file_a
        .imports
        .push(make_import("myapp.utils", "parse_token", "parse_token"));

    let graph = build_import_graph(&[file_a, file_b]);

    let ri = graph
        .lookup("myapp/auth.py", "parse_token")
        .expect("parse_token should resolve");
    assert_eq!(ri.source_file, "myapp/utils.py");
    assert!(matches!(ri.kind, ImportKind::Callable));
}

#[test]
fn python_module_import_resolves_to_module() {
    let file_b = make_file_record("myapp/config.py");

    let mut file_a = make_file_record("myapp/main.py");
    file_a.imports.push(Import {
        orig_module: "myapp.config".to_string(),
        orig_name: String::new(),
        module_alias: "myapp.config".to_string(),
        name_alias: String::new(),
        codeword: "myapp.config".to_string(),
    });

    let graph = build_import_graph(&[file_a, file_b]);

    let ri = graph
        .lookup("myapp/main.py", "myapp.config")
        .expect("module import should resolve");
    assert_eq!(ri.source_file, "myapp/config.py");
    assert!(matches!(ri.kind, ImportKind::Module));
}

#[test]
fn python_wildcard_import_is_skipped() {
    let file_b = make_file_record("myapp/utils.py");

    let mut file_a = make_file_record("myapp/main.py");
    file_a.imports.push(make_import("myapp.utils", "*", "*"));

    let graph = build_import_graph(&[file_a, file_b]);
    assert!(
        graph.resolved_imports.is_empty(),
        "wildcards must not be resolved"
    );
}

#[test]
fn third_party_import_is_not_resolved() {
    let mut file_a = make_file_record("myapp/main.py");
    file_a
        .imports
        .push(make_import("requests", "Session", "Session"));

    let graph = build_import_graph(&[file_a]);
    assert!(
        graph.resolved_imports.is_empty(),
        "third-party import should produce no resolution"
    );
}

#[test]
fn python_suffix_match_handles_project_root_prefix() {
    let mut file_b = make_file_record("src/myapp/services.py");
    file_b
        .entities
        .push(make_entity("OrderService", "src/myapp/services.py"));

    let mut file_a = make_file_record("src/myapp/main.py");
    file_a.imports.push(make_import(
        "myapp.services",
        "OrderService",
        "OrderService",
    ));

    let graph = build_import_graph(&[file_a, file_b]);

    assert!(
        graph.lookup("src/myapp/main.py", "OrderService").is_some(),
        "suffix match should resolve despite project root prefix"
    );
}

#[test]
fn python_enum_resolves_as_entity() {
    let mut file_b = make_file_record("myapp/status.py");
    file_b.enums.push(EnumDefinition {
        name: "Status".to_string(),
        variants: vec!["ACTIVE".to_string(), "INACTIVE".to_string()],
        file_path: "myapp/status.py".to_string(),
    });

    let mut file_a = make_file_record("myapp/main.py");
    file_a
        .imports
        .push(make_import("myapp.status", "Status", "Status"));

    let graph = build_import_graph(&[file_a, file_b]);

    let ri = graph
        .lookup("myapp/main.py", "Status")
        .expect("Status should resolve");
    assert!(matches!(ri.kind, ImportKind::Entity));
}

#[test]
fn empty_file_records_produces_empty_graph() {
    assert!(build_import_graph(&[]).resolved_imports.is_empty());
}

#[test]
fn file_with_no_imports_produces_empty_graph() {
    let record = make_file_record("myapp/main.py");
    assert!(build_import_graph(&[record]).resolved_imports.is_empty());
}

#[test]
fn python_relative_dot_import_resolves_across_packages() {
    // `from ....singletons import Settings` inside
    // `medical-data-service/medical_data_service/api/v3/annotation/jobs.py` must
    // anchor 4 dots at the importer's package and walk up to
    // `medical-data-service/medical_data_service/singletons.py`.
    let mut singletons =
        make_file_record("medical-data-service/medical_data_service/singletons.py");
    singletons.entities.push(make_entity(
        "Settings",
        "medical-data-service/medical_data_service/singletons.py",
    ));

    let mut jobs =
        make_file_record("medical-data-service/medical_data_service/api/v3/annotation/jobs.py");
    jobs.imports
        .push(make_import("....singletons", "Settings", "Settings"));

    let graph = build_import_graph(&[jobs, singletons]);

    let ri = graph
        .lookup(
            "medical-data-service/medical_data_service/api/v3/annotation/jobs.py",
            "Settings",
        )
        .expect("relative-dot import should resolve across packages");
    assert_eq!(
        ri.source_file,
        "medical-data-service/medical_data_service/singletons.py"
    );
    assert!(matches!(ri.kind, ImportKind::Entity));
}

#[test]
fn python_single_dot_relative_import_resolves_in_same_package() {
    // `from .helpers import params` must stay in the importer's package.
    let mut helpers = make_file_record("pkg/helpers.py");
    helpers
        .callables
        .push(make_callable("params()", "dict", "pkg/helpers.py"));

    let mut main = make_file_record("pkg/main.py");
    main.imports
        .push(make_import(".helpers", "params", "params"));

    let graph = build_import_graph(&[main, helpers]);

    let ri = graph
        .lookup("pkg/main.py", "params")
        .expect("single-dot relative import should resolve");
    assert_eq!(ri.source_file, "pkg/helpers.py");
}

#[test]
fn same_codeword_in_different_files_resolves_independently() {
    // Two microservices both have a class named Order, but under different modules,
    // which is the common real-world case. Each importer resolves to its own target.
    let mut svc_a_order = make_file_record("service-a/orders/models.py");
    svc_a_order
        .entities
        .push(make_entity("Order", "service-a/orders/models.py"));

    let mut svc_b_order = make_file_record("service-b/payments/models.py");
    svc_b_order
        .entities
        .push(make_entity("Order", "service-b/payments/models.py"));

    let mut svc_a_main = make_file_record("service-a/orders/main.py");
    svc_a_main
        .imports
        .push(make_import("orders.models", "Order", "Order"));

    let mut svc_b_main = make_file_record("service-b/payments/main.py");
    svc_b_main
        .imports
        .push(make_import("payments.models", "Order", "Order"));

    let graph = build_import_graph(&[svc_a_main, svc_b_main, svc_a_order, svc_b_order]);

    let ri_a = graph
        .lookup("service-a/orders/main.py", "Order")
        .expect("service-a Order should resolve");
    let ri_b = graph
        .lookup("service-b/payments/main.py", "Order")
        .expect("service-b Order should resolve");

    assert_eq!(ri_a.source_file, "service-a/orders/models.py");
    assert_eq!(ri_b.source_file, "service-b/payments/models.py");
}
