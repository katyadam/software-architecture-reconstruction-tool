use extractor_runtime::call_graph::{resolve_edges, resolve_edges_with_java_tests};
use models::call_graph::{
    CallGraphOutcome, CallGraphStatus, Language, MethodReachability, MethodRef, RawCallEdge,
};
use models::{Callable, JavaTestCase, Namespace, Parameter};

#[test]
fn configured_layout_resolves_nonstandard_roots_by_package_and_nested_owner() {
    use extractor_runtime::call_graph::{JavaSourceLayout, resolve_edges_with_java_layout};
    let method = |owner: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{owner}"),
        member_name: "run".into(),
        descriptor: "()V".into(),
        source_path: None,
        source_line: None,
    };
    let outcome = CallGraphOutcome {
        schema_version: 2,
        status: CallGraphStatus::Ok,
        provider_id: "wala-java".into(),
        source_root: "/project".into(),
        algorithm: "cha".into(),
        diagnostics: vec![],
        reachability: vec![],
        edges: vec![RawCallEdge {
            caller: method("a/Service"),
            callee: method("b/Service"),
            provider_id: "wala-java".into(),
            algorithm: "cha".into(),
            confidence: None,
        }],
    };
    let callables = [
        callable("Service", "/project/source/java/a/Service.java"),
        callable("Service", "/project/source/java/b/Service.java"),
    ];
    let layout = JavaSourceLayout {
        production_roots: vec!["/project/source/java".into()],
        test_roots: vec![],
    };
    let (edges, diagnostics) = resolve_edges_with_java_layout(
        &outcome,
        std::path::Path::new("/project"),
        &callables,
        &layout,
    );
    assert_eq!(edges.len(), 1, "{diagnostics:?}");
    let mut nested = outcome.clone();
    nested.edges[0].callee = method("b/Service$Inner");
    let nested_callables = [
        callables[0].clone(),
        callable("Inner", "/project/source/java/b/Service.java"),
    ];
    assert_eq!(
        resolve_edges_with_java_layout(
            &nested,
            std::path::Path::new("/project"),
            &nested_callables,
            &layout
        )
        .0
        .len(),
        1
    );
    let mut constructor = callable("Service", "/project/source/java/a/Service.java");
    constructor.is_constructor = true;
    constructor.name = "Service()".into();
    constructor.return_type = None;
    constructor.signature = "constructor-a".into();
    nested.edges[0].caller.member_name = "<init>".into();
    let edges = resolve_edges_with_java_layout(
        &nested,
        std::path::Path::new("/project"),
        &[constructor, nested_callables[1].clone()],
        &layout,
    )
    .0;
    assert_eq!(edges[0].source_id, "constructor-a");
}

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
        reachability: vec![],
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
fn resolves_same_named_java_classes_by_their_package_path() {
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
        reachability: vec![],
        edges: vec![RawCallEdge {
            caller: method("first/BitType"),
            callee: method("second/BitType"),
            provider_id: "wala-java".into(),
            algorithm: "x".into(),
            confidence: None,
        }],
    };

    let (edges, diagnostics) = resolve_edges(
        &outcome,
        std::path::Path::new("/service"),
        &[
            callable_named(
                "BitType",
                "run",
                "/service/src/main/java/first/BitType.java",
            ),
            callable_named(
                "BitType",
                "run",
                "/service/src/main/java/second/BitType.java",
            ),
        ],
    );

    assert_eq!(edges.len(), 1);
    assert!(diagnostics.is_empty());
}

#[test]
fn resolves_java_methods_with_generic_return_types() {
    let method = |class: &str, member_name: &str, descriptor: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: member_name.into(),
        descriptor: descriptor.into(),
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
        reachability: vec![],
        edges: vec![RawCallEdge {
            caller: method("fixture/Test", "coversChanged", "()V"),
            callee: method(
                "fixture/Service",
                "preCommit",
                "(Ljava/util/Map;)Ljava/util/Map;",
            ),
            provider_id: "wala-java".into(),
            algorithm: "x".into(),
            confidence: None,
        }],
    };
    let tests = vec![JavaTestCase {
        module_root: "/service".into(),
        class_name: "Lfixture/Test".into(),
        method_name: "coversChanged".into(),
        descriptor: "()V".into(),
        callable_signature: "Test.coversChanged()V".into(),
        source_path: "/service/src/test/java/fixture/Test.java".into(),
    }];
    let generic_callable = Callable {
        name: "Map<TopicPartition, OffsetAndMetadata> preCommit(Map<TopicPartition, OffsetAndMetadata> currentOffsets)".into(),
        signature: "class:Service/Map<TopicPartition, OffsetAndMetadata> preCommit(Map<TopicPartition, OffsetAndMetadata> currentOffsets)".into(),
        namespace: Namespace::Class("Service".into()),
        parameters: vec![Parameter::without_default_value(
            "currentOffsets".into(),
            "Map<TopicPartition, OffsetAndMetadata>".into(),
        )],
        return_type: Some("Map<TopicPartition, OffsetAndMetadata>".into()),
        is_async: false,
        is_constructor: false,
        hash: "service".into(),
        file_path: "/service/src/main/java/fixture/Service.java".into(),
    };

    let (edges, diagnostics) = resolve_edges_with_java_tests(
        &outcome,
        std::path::Path::new("/service"),
        &[generic_callable],
        &tests,
    );

    assert_eq!(edges.len(), 1);
    assert!(diagnostics.is_empty());
}

#[test]
fn resolves_java_overloads_by_jvm_parameter_count() {
    let method = |class: &str, member_name: &str, descriptor: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: member_name.into(),
        descriptor: descriptor.into(),
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
        reachability: vec![],
        edges: vec![RawCallEdge {
            caller: method("fixture/Caller", "run", "()V"),
            callee: method(
                "fixture/Service",
                "flush",
                "(Ljava/lang/String;Ljava/lang/Object;)V",
            ),
            provider_id: "wala-java".into(),
            algorithm: "x".into(),
            confidence: None,
        }],
    };
    let mut two_argument_overload = callable_named(
        "Service",
        "flush",
        "/service/src/main/java/fixture/Service.java",
    );
    two_argument_overload.parameters = vec![
        Parameter::without_default_value("first".into(), "String".into()),
        Parameter::without_default_value("second".into(), "Object".into()),
    ];
    let mut three_argument_overload = callable_named(
        "Service",
        "flush",
        "/service/src/main/java/fixture/Service.java",
    );
    three_argument_overload.parameters = vec![
        Parameter::without_default_value("first".into(), "String".into()),
        Parameter::without_default_value("second".into(), "Object".into()),
        Parameter::without_default_value("third".into(), "Object".into()),
    ];

    let (edges, diagnostics) = resolve_edges(
        &outcome,
        std::path::Path::new("/service"),
        &[
            callable("Caller", "/service/src/main/java/fixture/Caller.java"),
            two_argument_overload,
            three_argument_overload,
        ],
    );

    assert_eq!(edges.len(), 1);
    assert!(diagnostics.is_empty());
}

#[test]
fn resolves_same_arity_java_overloads_by_jvm_parameter_types() {
    let method = |class: &str, member_name: &str, descriptor: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: member_name.into(),
        descriptor: descriptor.into(),
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
        reachability: vec![],
        edges: vec![RawCallEdge {
            caller: method("fixture/Caller", "run", "()V"),
            callee: method("fixture/Service", "render", "(ILfixture/Size;)V"),
            provider_id: "wala-java".into(),
            algorithm: "x".into(),
            confidence: None,
        }],
    };
    let mut size_overload = callable_named(
        "Service",
        "render",
        "/service/src/main/java/fixture/Service.java",
    );
    size_overload.parameters = vec![
        Parameter::without_default_value("kind".into(), "int".into()),
        Parameter::without_default_value("size".into(), "Size".into()),
    ];
    let mut integer_overload = callable_named(
        "Service",
        "render",
        "/service/src/main/java/fixture/Service.java",
    );
    integer_overload.parameters = vec![
        Parameter::without_default_value("kind".into(), "int".into()),
        Parameter::without_default_value("length".into(), "int".into()),
    ];

    let (edges, diagnostics) = resolve_edges(
        &outcome,
        std::path::Path::new("/service"),
        &[
            callable("Caller", "/service/src/main/java/fixture/Caller.java"),
            size_overload,
            integer_overload,
        ],
    );

    assert_eq!(edges.len(), 1);
    assert!(diagnostics.is_empty());
}

#[test]
fn resolves_java_generic_type_variables_after_bytecode_erasure() {
    let method = |class: &str, member_name: &str, descriptor: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: member_name.into(),
        descriptor: descriptor.into(),
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
        reachability: vec![],
        edges: vec![RawCallEdge {
            caller: method("fixture/Caller", "run", "()V"),
            callee: method(
                "fixture/Transformation",
                "apply",
                "(Lorg/apache/kafka/connect/connector/ConnectRecord;)Lorg/apache/kafka/connect/connector/ConnectRecord;",
            ),
            provider_id: "wala-java".into(),
            algorithm: "x".into(),
            confidence: None,
        }],
    };
    let mut apply = callable_named(
        "Transformation",
        "apply",
        "/service/src/main/java/fixture/Transformation.java",
    );
    apply.parameters = vec![Parameter::without_default_value(
        "record".into(),
        "R".into(),
    )];

    let (edges, diagnostics) = resolve_edges(
        &outcome,
        std::path::Path::new("/service"),
        &[
            callable("Caller", "/service/src/main/java/fixture/Caller.java"),
            apply,
        ],
    );

    assert_eq!(edges.len(), 1);
    assert!(diagnostics.is_empty());
}

#[test]
fn resolves_source_reachability_to_its_discovered_test() {
    let method = |class: &str, member_name: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: member_name.into(),
        descriptor: "()V".into(),
        source_path: None,
        source_line: None,
    };
    let outcome = CallGraphOutcome {
        schema_version: 2,
        status: CallGraphStatus::Ok,
        provider_id: "wala-java".into(),
        source_root: "/service".into(),
        algorithm: "x".into(),
        diagnostics: vec![],
        reachability: vec![MethodReachability {
            method: method("fixture/Service", "changed"),
            entrypoints: vec![method("fixture/ServiceTest", "coversChanged")],
        }],
        edges: vec![],
    };
    let tests = vec![JavaTestCase {
        module_root: "/service".into(),
        class_name: "Lfixture/ServiceTest".into(),
        method_name: "coversChanged".into(),
        descriptor: "()V".into(),
        callable_signature: "ServiceTest.coversChanged()V".into(),
        source_path: "/service/src/test/java/fixture/ServiceTest.java".into(),
    }];

    let result = extractor_runtime::call_graph::resolve_reachability_with_java_tests(
        &outcome,
        std::path::Path::new("/service"),
        &[callable_named(
            "Service",
            "changed",
            "/service/src/main/java/fixture/Service.java",
        )],
        &tests,
    );

    assert_eq!(
        result
            .reachable_tests_by_callable
            .get("class:Service/void changed()"),
        Some(&vec!["Lfixture/ServiceTest#coversChanged()V".into()])
    );
    assert!(result.diagnostics.is_empty());
}

#[test]
fn resolves_nested_java_parameter_types_by_their_innermost_name() {
    let method = |class: &str, member_name: &str, descriptor: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: member_name.into(),
        descriptor: descriptor.into(),
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
        reachability: vec![],
        edges: vec![RawCallEdge {
            caller: method("fixture/Caller", "run", "()V"),
            callee: method(
                "fixture/Writer",
                "write",
                "(Lfixture/RecordWriter$SqlStatementInfo;)V",
            ),
            provider_id: "wala-java".into(),
            algorithm: "x".into(),
            confidence: None,
        }],
    };
    let mut write = callable_named(
        "Writer",
        "write",
        "/service/src/main/java/fixture/Writer.java",
    );
    write.parameters = vec![Parameter::without_default_value(
        "statementInfo".into(),
        "SqlStatementInfo".into(),
    )];

    let (edges, diagnostics) = resolve_edges(
        &outcome,
        std::path::Path::new("/service"),
        &[
            callable("Caller", "/service/src/main/java/fixture/Caller.java"),
            write,
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
        reachability: vec![],
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
        reachability: vec![],
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
        reachability: vec![],
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
fn ignores_edges_to_implicit_java_constructors_without_selection_uncertainty() {
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
        reachability: vec![],
        edges: vec![RawCallEdge {
            caller: method("fixture/ServiceTest", "coversChanged"),
            callee: method("fixture/Service", "<init>"),
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
        &[callable(
            "Service",
            "/service/src/main/java/fixture/Service.java",
        )],
        &tests,
    );

    assert!(edges.is_empty());
    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_edges_to_compiler_generated_enum_values_without_selection_uncertainty() {
    let method = |class: &str, member_name: &str, descriptor: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: member_name.into(),
        descriptor: descriptor.into(),
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
        reachability: vec![],
        edges: vec![RawCallEdge {
            caller: method("fixture/ServiceTest", "coversChanged", "()V"),
            callee: method("fixture/Mode", "values", "()[Lfixture/Mode;"),
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
        &[callable("Mode", "/service/src/main/java/fixture/Mode.java")],
        &tests,
    );

    assert!(edges.is_empty());
    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_lombok_generated_to_string_without_selection_uncertainty() {
    let root = tempfile::tempdir().expect("temporary source root is created");
    let source = root.path().join("src/main/java/fixture/OrderDAO.java");
    std::fs::create_dir_all(source.parent().expect("source has a parent"))
        .expect("source directory is created");
    std::fs::write(
        &source,
        "package fixture; import lombok.ToString; @ToString class OrderDAO { void declared() {} }",
    )
    .expect("source is written");
    let method = |class: &str, member_name: &str, descriptor: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: member_name.into(),
        descriptor: descriptor.into(),
        source_path: None,
        source_line: None,
    };
    let outcome = CallGraphOutcome {
        schema_version: 1,
        status: CallGraphStatus::Ok,
        provider_id: "wala-java".into(),
        source_root: root.path().to_string_lossy().into(),
        algorithm: "x".into(),
        diagnostics: vec![],
        reachability: vec![],
        edges: vec![RawCallEdge {
            caller: method("external/Test", "runs", "()V"),
            callee: method("fixture/OrderDAO", "toString", "()Ljava/lang/String;"),
            provider_id: "wala-java".into(),
            algorithm: "x".into(),
            confidence: None,
        }],
    };

    let (_, diagnostics) = resolve_edges(
        &outcome,
        root.path(),
        &[callable_named(
            "OrderDAO",
            "declared",
            source.to_string_lossy().as_ref(),
        )],
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn reports_unannotated_unmapped_to_string_as_selection_uncertainty() {
    let root = tempfile::tempdir().expect("temporary source root is created");
    let source = root.path().join("src/main/java/fixture/OrderDAO.java");
    std::fs::create_dir_all(source.parent().expect("source has a parent"))
        .expect("source directory is created");
    std::fs::write(
        &source,
        "package fixture; class OrderDAO { void declared() {} }",
    )
    .expect("source is written");
    let method = |class: &str, member_name: &str, descriptor: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: member_name.into(),
        descriptor: descriptor.into(),
        source_path: None,
        source_line: None,
    };
    let outcome = CallGraphOutcome {
        schema_version: 1,
        status: CallGraphStatus::Ok,
        provider_id: "wala-java".into(),
        source_root: root.path().to_string_lossy().into(),
        algorithm: "x".into(),
        diagnostics: vec![],
        reachability: vec![],
        edges: vec![RawCallEdge {
            caller: method("external/Test", "runs", "()V"),
            callee: method("fixture/OrderDAO", "toString", "()Ljava/lang/String;"),
            provider_id: "wala-java".into(),
            algorithm: "x".into(),
            confidence: None,
        }],
    };

    let (_, diagnostics) = resolve_edges(
        &outcome,
        root.path(),
        &[callable_named(
            "OrderDAO",
            "declared",
            source.to_string_lossy().as_ref(),
        )],
    );

    assert_eq!(diagnostics[0], "WALA unresolved source endpoints=1");
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
        reachability: vec![],
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
        vec![
            "WALA unresolved source endpoints=1",
            "WALA unresolved source mismatch: class_candidates=2; file_candidates=2; method_candidates=2; endpoints=1",
            "WALA unresolved source endpoint: LB#run()V",
        ]
    );
}

#[test]
fn aggregates_repeated_unresolved_endpoints_without_hiding_distinct_examples() {
    let method = |class: &str| MethodRef {
        language: Language::Java,
        declaring_type: format!("L{class}"),
        member_name: "run".into(),
        descriptor: "()V".into(),
        source_path: None,
        source_line: None,
    };
    let unresolved_edge = |callee| RawCallEdge {
        caller: method("A"),
        callee: method(callee),
        provider_id: "wala-java".into(),
        algorithm: "x".into(),
        confidence: None,
    };
    let outcome = CallGraphOutcome {
        schema_version: 1,
        status: CallGraphStatus::Ok,
        provider_id: "wala-java".into(),
        source_root: "/service".into(),
        algorithm: "x".into(),
        diagnostics: vec![],
        reachability: vec![],
        edges: vec![
            unresolved_edge("B"),
            unresolved_edge("B"),
            unresolved_edge("C"),
        ],
    };

    let (_, diagnostics) = resolve_edges(
        &outcome,
        std::path::Path::new("/service"),
        &[
            callable("A", "/service/A.java"),
            callable_named("B", "run", "/service/B.java"),
            callable_named("B", "run", "/service/B.java"),
            callable_named("C", "run", "/service/C.java"),
            callable_named("C", "run", "/service/C.java"),
        ],
    );

    assert_eq!(
        diagnostics,
        vec![
            "WALA unresolved source endpoints=3",
            "WALA unresolved source mismatch: class_candidates=2; file_candidates=2; method_candidates=2; endpoints=3",
            "WALA unresolved source endpoint: LB#run()V",
            "WALA unresolved source endpoint: LC#run()V",
        ]
    );
}
