use std::collections::HashMap;

use models::{
    Callable, Entity, Import, Namespace, ParsedCallable,
    ir::{ast::CallableAst, language::Language, project::ImportKind, syntax::FileRecord},
};
use statix::import_graph::build_import_graph;

fn make_file_record(file_path: &str) -> FileRecord {
    FileRecord {
        file_path: file_path.to_string(),
        language: Language::Java,
        imports: vec![],
        entities: vec![],
        endpoints: vec![],
        callables: vec![],
        call_statements: vec![],
        assignments: HashMap::new(),
        enums: vec![],
        raw_restcalls: vec![],
        raw_message_edges: vec![],
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
fn java_import_resolves_entity() {
    let mut file_b = make_file_record("com/example/service/UserService.java");
    file_b.entities.push(make_entity(
        "UserService",
        "com/example/service/UserService.java",
    ));

    let mut file_a = make_file_record("com/example/Main.java");
    file_a.imports.push(make_import(
        "com.example.service.UserService",
        "UserService",
        "UserService",
    ));

    let graph = build_import_graph(&[file_a, file_b]);

    let ri = graph
        .lookup("com/example/Main.java", "UserService")
        .expect("UserService should resolve");
    assert_eq!(ri.source_file, "com/example/service/UserService.java");
    assert!(matches!(ri.kind, ImportKind::Entity));
    assert!(ri.fully_qualified_name.contains("UserService"));
}

#[test]
fn java_import_resolves_callable() {
    let mut file_b = make_file_record("com/example/Utils.java");
    file_b.callables.push(make_callable(
        "hashText(String)",
        "String",
        "com/example/Utils.java",
    ));

    let mut file_a = make_file_record("com/example/Main.java");
    file_a
        .imports
        .push(make_import("com.example.Utils", "hashText", "hashText"));

    let graph = build_import_graph(&[file_a, file_b]);

    let ri = graph
        .lookup("com/example/Main.java", "hashText")
        .expect("hashText should resolve");
    assert_eq!(ri.source_file, "com/example/Utils.java");
    assert!(matches!(ri.kind, ImportKind::Callable));
}

#[test]
fn java_suffix_match_handles_project_root_prefix() {
    // Maven layout: file_path includes "src/main/java/", but the import path doesn't.
    let mut file_b = make_file_record("src/main/java/com/example/OrderService.java");
    file_b.entities.push(make_entity(
        "OrderService",
        "src/main/java/com/example/OrderService.java",
    ));

    let mut file_a = make_file_record("src/main/java/com/example/Main.java");
    file_a.imports.push(make_import(
        "com.example.OrderService",
        "OrderService",
        "OrderService",
    ));

    let graph = build_import_graph(&[file_a, file_b]);

    assert!(
        graph
            .lookup("src/main/java/com/example/Main.java", "OrderService")
            .is_some(),
        "suffix match should resolve despite Maven project layout prefix"
    );
}

#[test]
fn same_codeword_in_different_files_resolves_independently() {
    // Two microservices both have a class named Order, but under different packages,
    // which is the common real-world case. Each importer resolves to its own target.
    let mut order_svc_order = make_file_record("order-service/com/example/orders/Order.java");
    order_svc_order.entities.push(make_entity(
        "Order",
        "order-service/com/example/orders/Order.java",
    ));

    let mut payment_svc_order = make_file_record("payment-service/com/example/payments/Order.java");
    payment_svc_order.entities.push(make_entity(
        "Order",
        "payment-service/com/example/payments/Order.java",
    ));

    let mut order_svc_main = make_file_record("order-service/com/example/Main.java");
    order_svc_main
        .imports
        .push(make_import("com.example.orders.Order", "Order", "Order"));

    let mut payment_svc_main = make_file_record("payment-service/com/example/Main.java");
    payment_svc_main
        .imports
        .push(make_import("com.example.payments.Order", "Order", "Order"));

    let graph = build_import_graph(&[
        order_svc_main,
        payment_svc_main,
        order_svc_order,
        payment_svc_order,
    ]);

    let order_ri = graph
        .lookup("order-service/com/example/Main.java", "Order")
        .expect("order-service Order should resolve");
    let payment_ri = graph
        .lookup("payment-service/com/example/Main.java", "Order")
        .expect("payment-service Order should resolve");

    assert_eq!(
        order_ri.source_file,
        "order-service/com/example/orders/Order.java"
    );
    assert_eq!(
        payment_ri.source_file,
        "payment-service/com/example/payments/Order.java"
    );
}
