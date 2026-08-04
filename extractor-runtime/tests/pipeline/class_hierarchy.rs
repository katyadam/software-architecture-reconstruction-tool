use std::collections::HashMap;

use extractor_runtime::pipeline::build_project_ir;
use models::{
    Entity, Import,
    ir::{language::Language, syntax::FileRecord},
};

fn make_entity(name: &str, signature: &str, superclasses: Vec<&str>) -> Entity {
    Entity {
        name: name.to_string(),
        superclasses: superclasses.into_iter().map(str::to_string).collect(),
        fields: vec![],
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
        proto_services: vec![],
    }
}

#[test]
fn java_single_inheritance_via_import() {
    let mut base_file = make_file_record("com/example/base/BaseService.java", Language::Java);
    base_file.entities.push(make_entity(
        "BaseService",
        "com.example.base.BaseService",
        vec![],
    ));

    let mut child_file = make_file_record("com/example/order/OrderService.java", Language::Java);
    child_file.imports.push(make_import(
        "com.example.base.BaseService",
        "BaseService",
        "BaseService",
    ));
    child_file.entities.push(make_entity(
        "OrderService",
        "com.example.order.OrderService",
        vec!["BaseService"],
    ));

    let ir = build_project_ir(vec![child_file, base_file]);

    let parents = &ir.class_hierarchy.parents;
    let children = &ir.class_hierarchy.children;

    assert_eq!(
        parents.get("com.example.order.OrderService"),
        Some(&vec!["com.example.base.BaseService".to_string()]),
        "OrderService should have BaseService as parent"
    );
    assert_eq!(
        children.get("com.example.base.BaseService"),
        Some(&vec!["com.example.order.OrderService".to_string()]),
        "BaseService should have OrderService as child"
    );
}

#[test]
fn java_interface_in_superclasses_resolved() {
    let mut iface_file = make_file_record("com/example/api/Processable.java", Language::Java);
    iface_file.entities.push(make_entity(
        "Processable",
        "com.example.api.Processable",
        vec![],
    ));

    let mut impl_file = make_file_record("com/example/order/Order.java", Language::Java);
    impl_file.imports.push(make_import(
        "com.example.api.Processable",
        "Processable",
        "Processable",
    ));
    impl_file.entities.push(make_entity(
        "Order",
        "com.example.order.Order",
        vec!["Processable"],
    ));

    let ir = build_project_ir(vec![impl_file, iface_file]);

    let parents = &ir.class_hierarchy.parents;
    assert_eq!(
        parents.get("com.example.order.Order"),
        Some(&vec!["com.example.api.Processable".to_string()]),
        "Order should have Processable as parent"
    );
    assert!(
        ir.class_hierarchy
            .children
            .get("com.example.api.Processable")
            .map(|v| v.contains(&"com.example.order.Order".to_string()))
            .unwrap_or(false),
        "Processable should have Order as child"
    );
}

#[test]
fn python_multiple_inheritance() {
    let mut mixin_file = make_file_record("myapp/mixins.py", Language::Python);
    mixin_file
        .entities
        .push(make_entity("LogMixin", "myapp/mixins.py/LogMixin", vec![]));

    let mut base_file = make_file_record("myapp/base.py", Language::Python);
    base_file
        .entities
        .push(make_entity("BaseModel", "myapp/base.py/BaseModel", vec![]));

    let mut child_file = make_file_record("myapp/models.py", Language::Python);
    child_file
        .imports
        .push(make_import("myapp.mixins", "LogMixin", "LogMixin"));
    child_file
        .imports
        .push(make_import("myapp.base", "BaseModel", "BaseModel"));
    child_file.entities.push(make_entity(
        "User",
        "myapp/models.py/User",
        vec!["BaseModel", "LogMixin"],
    ));

    let ir = build_project_ir(vec![child_file, mixin_file, base_file]);

    let parents = ir
        .class_hierarchy
        .parents
        .get("myapp/models.py/User")
        .expect("User should have parents");

    assert!(
        parents.contains(&"myapp/base.py/BaseModel".to_string()),
        "User parents should include BaseModel"
    );
    assert!(
        parents.contains(&"myapp/mixins.py/LogMixin".to_string()),
        "User parents should include LogMixin"
    );
    assert_eq!(parents.len(), 2, "User should have exactly 2 parents");
}

#[test]
fn unresolvable_superclass_skipped() {
    // java.io.Serializable is stdlib — no import in file, not defined in project
    let mut file = make_file_record("com/example/Foo.java", Language::Java);
    file.entities
        .push(make_entity("Foo", "com.example.Foo", vec!["Serializable"]));

    let ir = build_project_ir(vec![file]);

    assert!(
        ir.class_hierarchy.parents.get("com.example.Foo").is_none(),
        "unresolvable superclass should not appear in parents map"
    );
}

#[test]
fn same_file_superclass_resolved() {
    let mut file = make_file_record("com/example/domain/Domain.java", Language::Java);
    file.entities
        .push(make_entity("Base", "com.example.domain.Base", vec![]));
    file.entities.push(make_entity(
        "Derived",
        "com.example.domain.Derived",
        vec!["Base"],
    ));

    let ir = build_project_ir(vec![file]);

    assert_eq!(
        ir.class_hierarchy.parents.get("com.example.domain.Derived"),
        Some(&vec!["com.example.domain.Base".to_string()]),
        "Derived should resolve Base from same file"
    );
    assert_eq!(
        ir.class_hierarchy.children.get("com.example.domain.Base"),
        Some(&vec!["com.example.domain.Derived".to_string()]),
        "Base should list Derived as child"
    );
}
