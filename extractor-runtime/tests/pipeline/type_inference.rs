use std::collections::HashMap;

use extractor_runtime::pipeline::build_project_ir;
use models::{
    Argument, Assignment, AssignmentKey, CallStatement, Scope,
    ir::{language::Language, syntax::FileRecord},
};

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
    }
}

fn make_argument(value: &str) -> Argument {
    Argument {
        assigned_variable: String::new(),
        value: value.to_string(),
        datatype: None,
    }
}

fn make_call_statement(
    function_name: &str,
    enclosing_function: Option<&str>,
    enclosing_class: Option<&str>,
    arguments: Vec<Argument>,
) -> CallStatement {
    CallStatement {
        function_name: function_name.to_string(),
        arguments,
        enclosing_function_name: enclosing_function.map(str::to_string),
        enclosing_class_name: enclosing_class.map(str::to_string),
        enclosing_function_hash: None,
        is_self_invoke: false,
        is_super_invoke: false,
        invoked_on: None,
        is_decorator: false,
    }
}

fn make_assignment_key(scope: Scope, variable_name: &str) -> AssignmentKey {
    AssignmentKey {
        scope,
        variable_name: variable_name.to_string(),
    }
}

fn make_assignment(variable_name: &str, variable_type: &str, value: &str) -> Assignment {
    Assignment {
        variable_name: variable_name.to_string(),
        variable_type: variable_type.to_string(),
        value: value.to_string(),
    }
}

// --- Java ---

/// `invoked_on` is resolved via a local assignment in a Java file.
/// `orderService.getOrders()` -> `OrderService` because there is an
/// assignment `OrderService orderService = ...` in the enclosing function.
#[test]
fn java_invoked_on_resolved_via_assignment() {
    let mut file = make_file_record("com/example/Checkout.java", Language::Java);
    file.assignments.insert(
        make_assignment_key(
            Scope::Function("processOrder()".to_string()),
            "orderService",
        ),
        make_assignment("orderService", "OrderService", "new OrderService()"),
    );
    file.call_statements.push(make_call_statement(
        "orderService.getOrders()",
        Some("processOrder()"),
        None,
        vec![],
    ));

    let ir = build_project_ir(vec![file]);

    let call = &ir.files[0].call_statements[0];
    assert_eq!(
        call.invoked_on.as_deref(),
        Some("OrderService"),
        "invoked_on should resolve to OrderService via local assignment"
    );
}

/// `Argument.datatype` is resolved via a local variable assignment in a Java file.
/// `service.createOrder(myOrder)` -> argument datatype = "Order" because
/// `Order myOrder = new Order()` is in the assignments map.
#[test]
fn java_argument_datatype_resolved_via_local_assignment() {
    let mut file = make_file_record("com/example/Service.java", Language::Java);
    file.assignments.insert(
        make_assignment_key(Scope::Function("handle()".to_string()), "myOrder"),
        make_assignment("myOrder", "Order", "new Order()"),
    );
    file.call_statements.push(make_call_statement(
        "service.createOrder",
        Some("handle()"),
        None,
        vec![make_argument("myOrder")],
    ));

    let ir = build_project_ir(vec![file]);

    let arg = &ir.files[0].call_statements[0].arguments[0];
    assert_eq!(
        arg.datatype.as_deref(),
        Some("Order"),
        "argument datatype should resolve to Order via local assignment"
    );
}

/// Java: call with three arguments, each resolved from a separate local assignment.
#[test]
fn java_multiple_arguments_all_resolved() {
    let mut file = make_file_record("com/example/OrderHandler.java", Language::Java);
    let scope = Scope::Function("handleRequest()".to_string());

    file.assignments.insert(
        make_assignment_key(scope.clone(), "customer"),
        make_assignment("customer", "Customer", "new Customer()"),
    );
    file.assignments.insert(
        make_assignment_key(scope.clone(), "product"),
        make_assignment("product", "Product", "new Product()"),
    );
    file.assignments.insert(
        make_assignment_key(scope, "quantity"),
        make_assignment("quantity", "int", "10"),
    );
    file.call_statements.push(make_call_statement(
        "orderService.createOrder()",
        Some("handleRequest()"),
        None,
        vec![
            make_argument("customer"),
            make_argument("product"),
            make_argument("quantity"),
        ],
    ));

    let ir = build_project_ir(vec![file]);

    let args = &ir.files[0].call_statements[0].arguments;
    assert_eq!(args[0].datatype.as_deref(), Some("Customer"));
    assert_eq!(args[1].datatype.as_deref(), Some("Product"));
    assert_eq!(args[2].datatype.as_deref(), Some("int"));
}

/// Cross-file global: `BASE_URL` defined in file A resolves as an argument
/// datatype in file B's call.
#[test]
fn java_argument_datatype_resolved_via_cross_file_global() {
    let mut config_file = make_file_record("com/example/Config.java", Language::Java);
    config_file.assignments.insert(
        make_assignment_key(Scope::Global, "BASE_URL"),
        make_assignment("BASE_URL", "String", "\"http://example.com\""),
    );

    let mut client_file = make_file_record("com/example/Client.java", Language::Java);
    client_file.call_statements.push(make_call_statement(
        "httpClient.get",
        Some("fetchData()"),
        None,
        vec![make_argument("BASE_URL")],
    ));

    let ir = build_project_ir(vec![config_file, client_file]);

    let client_file_ir = ir
        .files
        .iter()
        .find(|f| f.file_path.contains("Client"))
        .expect("Client file should be in IR");
    let arg = &client_file_ir.call_statements[0].arguments[0];
    assert_eq!(
        arg.datatype.as_deref(),
        Some("String"),
        "BASE_URL argument should resolve to String via cross-file global"
    );
}

/// Unresolvable call leaves `invoked_on` as `None`.
#[test]
fn java_unresolvable_call_leaves_invoked_on_none() {
    let mut file = make_file_record("com/example/Foo.java", Language::Java);
    file.call_statements.push(make_call_statement(
        "unknownObj.method",
        Some("run()"),
        None,
        vec![],
    ));

    let ir = build_project_ir(vec![file]);

    let call = &ir.files[0].call_statements[0];
    assert!(
        call.invoked_on.is_none(),
        "invoked_on should remain None when no assignment matches"
    );
}

// --- Python ---

/// Python: `invoked_on` resolved via a local assignment.
/// `order_service.place_order` -> `OrderService` because there is an
/// assignment `order_service: OrderService = OrderService()` in the function.
#[test]
fn python_invoked_on_resolved_via_assignment() {
    let mut file = make_file_record("shop/checkout.py", Language::Python);
    file.assignments.insert(
        make_assignment_key(
            Scope::Function("process_order(order_id)".to_string()),
            "order_service",
        ),
        make_assignment("order_service", "OrderService", "OrderService()"),
    );
    file.call_statements.push(make_call_statement(
        "order_service.place_order",
        Some("process_order(order_id)"),
        None,
        vec![],
    ));

    let ir = build_project_ir(vec![file]);

    let call = &ir.files[0].call_statements[0];
    assert_eq!(
        call.invoked_on.as_deref(),
        Some("OrderService"),
        "invoked_on should resolve to OrderService via local assignment"
    );
}

/// Python: `Argument.datatype` resolved via a local variable assignment.
#[test]
fn python_argument_datatype_resolved_via_local_assignment() {
    let mut file = make_file_record("shop/service.py", Language::Python);
    file.assignments.insert(
        make_assignment_key(Scope::Function("handle(request)".to_string()), "invoice"),
        make_assignment("invoice", "Invoice", "Invoice()"),
    );
    file.call_statements.push(make_call_statement(
        "repo.save",
        Some("handle(request)"),
        None,
        vec![make_argument("invoice")],
    ));

    let ir = build_project_ir(vec![file]);

    let arg = &ir.files[0].call_statements[0].arguments[0];
    assert_eq!(
        arg.datatype.as_deref(),
        Some("Invoice"),
        "argument datatype should resolve to Invoice via local assignment"
    );
}

/// Python: call with two arguments, each resolved from a separate local assignment.
#[test]
fn python_multiple_arguments_all_resolved() {
    let mut file = make_file_record("shop/handler.py", Language::Python);
    let scope = Scope::Function("submit(self)".to_string());

    file.assignments.insert(
        make_assignment_key(scope.clone(), "entity"),
        make_assignment("entity", "Order", "Order()"),
    );
    file.assignments.insert(
        make_assignment_key(scope, "ctx"),
        make_assignment("ctx", "Context", "Context()"),
    );
    file.call_statements.push(make_call_statement(
        "repo.save",
        Some("submit(self)"),
        None,
        vec![make_argument("entity"), make_argument("ctx")],
    ));

    let ir = build_project_ir(vec![file]);

    let args = &ir.files[0].call_statements[0].arguments;
    assert_eq!(args[0].datatype.as_deref(), Some("Order"));
    assert_eq!(args[1].datatype.as_deref(), Some("Context"));
}

/// Python: unresolvable call leaves `invoked_on` as `None`.
#[test]
fn python_unresolvable_call_leaves_invoked_on_none() {
    let mut file = make_file_record("shop/foo.py", Language::Python);
    file.call_statements.push(make_call_statement(
        "unknown_obj.method",
        Some("run()"),
        None,
        vec![],
    ));

    let ir = build_project_ir(vec![file]);

    let call = &ir.files[0].call_statements[0];
    assert!(
        call.invoked_on.is_none(),
        "invoked_on should remain None when no assignment matches"
    );
}
