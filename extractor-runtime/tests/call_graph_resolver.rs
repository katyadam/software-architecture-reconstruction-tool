use extractor_runtime::call_graph::resolve_edges;
use models::call_graph::{CallGraphOutcome, CallGraphStatus, Language, MethodRef, RawCallEdge};
use models::{Callable, Namespace};

fn callable(class: &str, file: &str) -> Callable {
    Callable {
        name: "void run()".into(),
        signature: format!("class:{class}/void run()"),
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
