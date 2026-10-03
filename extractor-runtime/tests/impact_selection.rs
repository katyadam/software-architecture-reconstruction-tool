use extractor_runtime::impact::select_tests;
use models::{
    ChangedElement, ChangedElementKind, JavaTestCase, SelectionReason,
    call_graph::{CallGraphOutcome, CallGraphRequest, CallGraphStatus, ResolvedCallEdge},
};

/// Builds a test identity suitable for focused selection assertions.
fn test_case(name: &str) -> JavaTestCase {
    JavaTestCase {
        module_root: "orders".into(),
        class_name: format!("Lorders/{name}Test"),
        method_name: "coversChange".into(),
        descriptor: "()V".into(),
        callable_signature: format!("class:{name}Test/void coversChange()"),
        source_path: format!("src/test/java/orders/{name}Test.java"),
    }
}

/// Builds a callable change in the orders module.
fn callable_change() -> ChangedElement {
    ChangedElement {
        module_root: "orders".into(),
        source_path: "src/main/java/orders/OrderService.java".into(),
        kind: ChangedElementKind::Callable,
        callable_signature: Some("class:OrderService/void changed()".into()),
    }
}

/// Builds a successful provider outcome without uncertainty diagnostics.
fn successful_outcome() -> CallGraphOutcome {
    CallGraphOutcome::ok(
        CallGraphRequest::new("orders/src/main/java", models::call_graph::Language::Java),
        "wala-java",
        "cha_bytecode",
    )
}

#[test]
fn selects_only_the_test_that_reaches_a_changed_callable() {
    let reaching = test_case("OrderService");
    let unrelated = test_case("Inventory");

    let result = select_tests(
        vec![callable_change()],
        vec![reaching.clone(), unrelated.clone()],
        successful_outcome(),
        vec![ResolvedCallEdge {
            source_id: reaching.callable_signature.clone(),
            target_id: "class:OrderService/void changed()".into(),
        }],
    );

    assert_eq!(result.selected_tests.len(), 1);
    assert_eq!(result.selected_tests[0].test, reaching);
    assert_eq!(
        result.selected_tests[0].reasons,
        vec![SelectionReason::ChangedCallable {
            signature: "class:OrderService/void changed()".into(),
        }]
    );
    assert_eq!(result.unselected_tests, vec![unrelated]);
}

#[test]
fn falls_back_to_every_module_test_for_module_changes_and_analysis_uncertainty() {
    let first = test_case("OrderService");
    let second = test_case("Inventory");
    let mut failed = successful_outcome();
    failed.status = CallGraphStatus::Failed;
    failed
        .diagnostics
        .push("test bytecode compilation failed".into());
    let module_change = ChangedElement {
        module_root: "orders".into(),
        source_path: "pom.xml".into(),
        kind: ChangedElementKind::Module,
        callable_signature: None,
    };

    let result = select_tests(vec![module_change], vec![first, second], failed, vec![]);

    assert_eq!(result.selected_tests.len(), 2);
    assert!(result.unselected_tests.is_empty());
    assert!(result.selected_tests.iter().all(|selected| {
        selected
            .reasons
            .iter()
            .any(|reason| matches!(reason, SelectionReason::ModuleFallback { .. }))
            && selected
                .reasons
                .iter()
                .any(|reason| matches!(reason, SelectionReason::AnalysisFailure { .. }))
    }));
}

#[test]
fn falls_back_when_a_changed_callable_cannot_be_reached_or_mapped() {
    let first = test_case("OrderService");
    let second = test_case("Inventory");

    let result = select_tests(
        vec![callable_change()],
        vec![first, second],
        successful_outcome(),
        vec![],
    );

    assert_eq!(result.selected_tests.len(), 2);
    assert!(result.selected_tests.iter().all(|selected| {
        selected
            .reasons
            .iter()
            .any(|reason| matches!(reason, SelectionReason::ModuleFallback { .. }))
    }));
}

#[test]
fn selects_tests_reaching_any_callable_in_a_changed_class() {
    let reaching = test_case("OrderService");
    let class_change = ChangedElement {
        module_root: "orders".into(),
        source_path: "src/main/java/orders/OrderService.java".into(),
        kind: ChangedElementKind::Class,
        callable_signature: Some("OrderService".into()),
    };

    let result = select_tests(
        vec![class_change],
        vec![reaching.clone()],
        successful_outcome(),
        vec![ResolvedCallEdge {
            source_id: reaching.callable_signature.clone(),
            target_id: "class:OrderService/void unchangedMethod()".into(),
        }],
    );

    assert_eq!(result.selected_tests.len(), 1);
    assert_eq!(
        result.selected_tests[0].reasons,
        vec![SelectionReason::ChangedClass {
            class_name: "OrderService".into(),
        }]
    );
}

#[test]
fn falls_back_when_wala_reports_an_unmatched_or_unresolved_mapping_diagnostic() {
    let first = test_case("OrderService");
    let second = test_case("Inventory");
    let mut outcome = successful_outcome();
    outcome
        .diagnostics
        .push("unmatched_entrypoint=Lorders/OrderServiceTest#coversChange()V".into());

    let result = select_tests(
        vec![callable_change()],
        vec![first, second],
        outcome,
        vec![],
    );

    assert_eq!(result.selected_tests.len(), 2);
    assert!(result.selected_tests.iter().all(|selected| {
        selected
            .reasons
            .iter()
            .any(|reason| matches!(reason, SelectionReason::ModuleFallback { .. }))
    }));
}
