use std::collections::HashMap;

use extractor_runtime::pipeline::build_project_ir;
use models::{
    Entity, Field, Import,
    ir::{language::Language, syntax::FileRecord},
};

fn make_field(name: &str, datatype: &str) -> Field {
    Field {
        name: name.to_string(),
        datatype: Some(datatype.to_string()),
        initial_value: None,
        datatype_signature: None,
        is_collection: false,
    }
}

fn make_entity(name: &str, signature: &str, fields: Vec<Field>) -> Entity {
    Entity {
        name: name.to_string(),
        superclasses: vec![],
        fields,
        signature: signature.to_string(),
        file_path: String::new(),
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

fn make_file_record(file_path: &str, language: Language) -> FileRecord {
    FileRecord {
        file_path: file_path.to_string(),
        language,
        imports: vec![],
        entities: vec![],
        endpoints: vec![],
        callables: vec![],
        call_statements: vec![],
        assignments: HashMap::new(),
        enums: vec![],
        raw_restcalls: vec![],
    }
}

#[test]
fn java_field_resolved_via_import() {
    let mut order_file = make_file_record("com/example/order/Order.java", Language::Java);
    order_file
        .entities
        .push(make_entity("Order", "com.example.order.Order", vec![]));

    let mut checkout_file = make_file_record("com/example/checkout/Checkout.java", Language::Java);
    checkout_file
        .imports
        .push(make_import("com.example.order.Order", "Order", "Order"));
    checkout_file.entities.push(make_entity(
        "Checkout",
        "com.example.checkout.Checkout",
        vec![make_field("order", "Order")],
    ));

    let ir = build_project_ir(vec![checkout_file, order_file]);

    let sig = ir.files[0].entities[0].fields[0]
        .datatype_signature
        .as_deref()
        .expect("datatype_signature should be resolved");
    assert_eq!(sig, "com.example.order.Order");
}

#[test]
fn python_field_resolved_via_import() {
    let mut model_file = make_file_record("myapp/models.py", Language::Python);
    model_file
        .entities
        .push(make_entity("User", "myapp/models.py/User", vec![]));

    let mut service_file = make_file_record("myapp/service.py", Language::Python);
    service_file
        .imports
        .push(make_import("models", "User", "User"));
    service_file.entities.push(make_entity(
        "UserService",
        "myapp/service.py/UserService",
        vec![make_field("user", "User")],
    ));

    let ir = build_project_ir(vec![service_file, model_file]);

    let sig = ir.files[0].entities[0].fields[0]
        .datatype_signature
        .as_deref()
        .expect("datatype_signature should be resolved");
    assert!(
        sig.contains("User"),
        "signature should reference User, got: {sig}"
    );
}

#[test]
fn java_generic_field_resolved() {
    let mut item_file = make_file_record("com/example/Item.java", Language::Java);
    item_file
        .entities
        .push(make_entity("Item", "com.example.Item", vec![]));

    let mut cart_file = make_file_record("com/example/Cart.java", Language::Java);
    cart_file
        .imports
        .push(make_import("com.example.Item", "Item", "Item"));
    cart_file.entities.push(make_entity(
        "Cart",
        "com.example.Cart",
        vec![make_field("items", "List<Item>")],
    ));

    let ir = build_project_ir(vec![cart_file, item_file]);

    let sig = ir.files[0].entities[0].fields[0]
        .datatype_signature
        .as_deref()
        .expect("List<Item> field should resolve");
    assert_eq!(sig, "com.example.Item");
}

#[test]
fn field_resolved_via_local_entity() {
    let mut file = make_file_record("com/example/User.java", Language::Java);
    file.entities
        .push(make_entity("Address", "com.example.Address", vec![]));
    file.entities.push(make_entity(
        "User",
        "com.example.User",
        vec![make_field("address", "Address")],
    ));

    let ir = build_project_ir(vec![file]);

    let user_entity = ir.files[0]
        .entities
        .iter()
        .find(|e| e.name == "User")
        .expect("User entity should exist");
    let sig = user_entity.fields[0]
        .datatype_signature
        .as_deref()
        .expect("Address field should resolve to local entity");
    assert_eq!(sig, "com.example.Address");
}

#[test]
fn java_fqdn_field_used_as_is() {
    let mut file = make_file_record("com/example/Foo.java", Language::Java);
    file.entities.push(make_entity(
        "Foo",
        "com.example.Foo",
        vec![make_field("bar", "com.example.other.Bar")],
    ));

    let ir = build_project_ir(vec![file]);

    assert_eq!(
        ir.files[0].entities[0].fields[0]
            .datatype_signature
            .as_deref(),
        Some("com.example.other.Bar")
    );
}

#[test]
fn unknown_type_leaves_signature_none() {
    let mut file = make_file_record("com/example/Foo.java", Language::Java);
    file.entities.push(make_entity(
        "Foo",
        "com.example.Foo",
        vec![make_field("bar", "UnknownType")],
    ));

    let ir = build_project_ir(vec![file]);

    assert!(
        ir.files[0].entities[0].fields[0]
            .datatype_signature
            .is_none(),
        "unresolvable type should leave datatype_signature as None"
    );
}
