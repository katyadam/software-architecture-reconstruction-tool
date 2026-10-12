use extractor_runtime::call_graph::{resolve_edges, resolve_edges_with_java_tests};
use models::call_graph::{CallGraphOutcome, CallGraphStatus, Language, MethodRef, RawCallEdge};
use models::{Callable, JavaTestCase, Namespace};

fn callable(class: &str, file: &str) -> Callable {
    callable_named(class, "run", file)
}

fn callable_named(class: &str, method: &str, file: &str) -> Callable {
    Callable {
        name: format!("void {method}()"),
        signature: format!("class:{class}/void {method}()"),
        namespace: Namespace::Class(class.into()),
        parameters: vec![],
        return_type: Some("void".into()),
        is_async: false,
        is_constructor: false,
        hash: class.into(),
        file_path: file.into(),
    }
}
#[test]
fn resolves_only_the_unique_matching_class_and_method() {
    let method = |class| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: "run".into(),
        descriptor: "()V".into(),
        source_path: None,
        source_line: None,
    };
    let outcome = CallGraphOutcome {
        schema_version: 1,
        status: CallGraphStatus::Ok,
        provider_id: "wala-java".into(),
        source_root: "/service".into(),
        algorithm: "x".into(),
        diagnostics: vec![],
        edges: vec![RawCallEdge {
            caller: method("A"),
            callee: method("B"),
            provider_id: "wala-java".into(),
            algorithm: "x".into(),
            confidence: None,
        }],
    };
    let (edges, diagnostics) = resolve_edges(
        &outcome,
        std::path::Path::new("/service"),
        &[
            callable("A", "/service/A.java"),
            callable("B", "/service/B.java"),
        ],
    );
    assert_eq!(edges.len(), 1);
    assert!(diagnostics.is_empty());
}

#[test]
fn resolves_a_wala_test_entrypoint_without_extracting_test_source_callables() {
    let method = |class: &str, member_name: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: member_name.into(),
        descriptor: "()V".into(),
        source_path: None,
        source_line: None,
    };
    let outcome = CallGraphOutcome {
        schema_version: 1,
        status: CallGraphStatus::Ok,
        provider_id: "wala-java".into(),
        source_root: "/service".into(),
        algorithm: "x".into(),
        diagnostics: vec![],
        edges: vec![RawCallEdge {
            caller: method("fixture/ServiceTest", "coversChanged"),
            callee: method("fixture/Service", "run"),
            provider_id: "wala-java".into(),
            algorithm: "x".into(),
            confidence: None,
        }],
    };
    let tests = vec![JavaTestCase {
        module_root: "/service".into(),
        class_name: "Lfixture/ServiceTest".into(),
        method_name: "coversChanged".into(),
        descriptor: "()V".into(),
        callable_signature: "ServiceTest.coversChanged()V".into(),
        source_path: "/service/src/test/java/fixture/ServiceTest.java".into(),
    }];

    let (edges, diagnostics) = resolve_edges_with_java_tests(
        &outcome,
        std::path::Path::new("/service"),
        &[
            callable("Service", "/service/src/main/java/fixture/Service.java"),
            callable_named(
                "ServiceTest",
                "coversChanged",
                "/service/src/main/java/other/ServiceTest.java",
            ),
        ],
        &tests,
    );

    assert_eq!(
        edges,
        vec![models::call_graph::ResolvedCallEdge {
            source_id: "Lfixture/ServiceTest#coversChanged()V".into(),
            target_id: "class:Service/void run()".into(),
        }]
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_resolve_a_test_from_a_sibling_module_with_a_shared_string_prefix() {
    let method = |class: &str, member_name: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: member_name.into(),
        descriptor: "()V".into(),
        source_path: None,
        source_line: None,
    };
    let outcome = CallGraphOutcome {
        schema_version: 1,
        status: CallGraphStatus::Ok,
        provider_id: "wala-java".into(),
        source_root: "/repo/orders".into(),
        algorithm: "x".into(),
        diagnostics: vec![],
        edges: vec![
            RawCallEdge {
                caller: method("fixture/ServiceTest", "coversChanged"),
                callee: method("fixture/Service", "run"),
                provider_id: "wala-java".into(),
                algorithm: "x".into(),
                confidence: None,
            };
            2
        ],
    };
    let tests = vec![JavaTestCase {
        module_root: "/repo/orders2".into(),
        class_name: "Lfixture/ServiceTest".into(),
        method_name: "coversChanged".into(),
        descriptor: "()V".into(),
        callable_signature: "ServiceTest.coversChanged()V".into(),
        source_path: "/repo/orders2/src/test/java/fixture/ServiceTest.java".into(),
    }];

    let (edges, diagnostics) = resolve_edges_with_java_tests(
        &outcome,
        std::path::Path::new("/repo/orders"),
        &[callable(
            "Service",
            "/repo/orders/src/main/java/fixture/Service.java",
        )],
        &tests,
    );

    assert!(edges.is_empty());
    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_edges_to_external_classes_without_causing_selection_uncertainty() {
    let method = |class: &str, member_name: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: member_name.into(),
        descriptor: "()V".into(),
        source_path: None,
        source_line: None,
    };
    let outcome = CallGraphOutcome {
        schema_version: 1,
        status: CallGraphStatus::Ok,
        provider_id: "wala-java".into(),
        source_root: "/service".into(),
        algorithm: "x".into(),
        diagnostics: vec![],
        edges: vec![RawCallEdge {
            caller: method("fixture/Service", "run"),
            callee: method("java/util/List", "size"),
            provider_id: "wala-java".into(),
            algorithm: "x".into(),
            confidence: None,
        }],
    };

    let (edges, diagnostics) = resolve_edges(
        &outcome,
        std::path::Path::new("/service"),
        &[callable(
            "Service",
            "/service/src/main/java/fixture/Service.java",
        )],
    );

    assert!(edges.is_empty());
    assert!(diagnostics.is_empty());
}

#[test]
fn reports_ambiguous_module_methods_as_selection_uncertainty() {
    let method = |class: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: "run".into(),
        descriptor: "()V".into(),
        source_path: None,
        source_line: None,
    };
    let outcome = CallGraphOutcome {
        schema_version: 1,
        status: CallGraphStatus::Ok,
        provider_id: "wala-java".into(),
        source_root: "/service".into(),
        algorithm: "x".into(),
        diagnostics: vec![],
        edges: vec![RawCallEdge {
            caller: method("A"),
            callee: method("B"),
            provider_id: "wala-java".into(),
            algorithm: "x".into(),
            confidence: None,
        }],
    };

    let (_, diagnostics) = resolve_edges(
        &outcome,
        std::path::Path::new("/service"),
        &[
            callable("A", "/service/A.java"),
            callable_named("B", "run", "/service/B.java"),
            callable_named("B", "run", "/service/B.java"),
        ],
    );

    assert_eq!(
        diagnostics,
        vec!["WALA edge could not be mapped uniquely to source callables"]
    );
}
