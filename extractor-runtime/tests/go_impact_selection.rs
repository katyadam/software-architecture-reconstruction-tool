use extractor_runtime::{call_graph::go_native::GoAnalysisResult, impact::go::select_go_tests};
use models::{
    call_graph::{
        CallGraphOutcome, CallGraphRequest, CallGraphStatus, Language, MethodReachability,
        MethodRef,
    },
    go_impact::*,
};
fn fixture() -> (Vec<GoChangedElement>, GoInventoryOutcome, GoAnalysisResult) {
    let tests: Vec<_> = (0..7)
        .map(|n| GoTestCase {
            method: MethodRef {
                language: Language::Go,
                declaring_type: "p".into(),
                member_name: format!("Test{n}"),
                descriptor: "func(*testing.T)".into(),
                source_path: Some("/p/s_test.go".into()),
                source_line: Some(n + 1),
            },
            execution_package: "p".into(),
        })
        .collect();
    let changed = GoChangedElement {
        source_path: "s.go".into(),
        old_path: Some("s.go".into()),
        kind: GoChangeKind::Callable,
        target: Some("p#Changed".into()),
        diagnostics: vec![],
    };
    let scope = GoAnalysisScope {
        dependency_bodies: "opaque".into(),
        goos: "linux".into(),
        goarch: "amd64".into(),
        go_version: "go1.26".into(),
        build_tags: "".into(),
        workspace_mode: "off".into(),
        limitations: vec![],
    };
    let inventory = GoInventoryOutcome {
        schema_version: 1,
        provider_id: "go-native".into(),
        status: GoStageStatus::Ok,
        source_root: "/p".into(),
        source_files: vec![],
        tests: tests.clone(),
        declarations: vec![],
        test_inventory_complete: true,
        scope: Some(scope.clone()),
        diagnostics: vec![],
    };
    let mut graph = CallGraphOutcome::ok(
        CallGraphRequest::new("/p", Language::Go),
        "go-native",
        "cha",
    );
    graph.schema_version = 2;
    let method = MethodRef {
        language: Language::Go,
        declaring_type: "p".into(),
        member_name: "Changed".into(),
        descriptor: "func()".into(),
        source_path: Some("/p/s.go".into()),
        source_line: Some(1),
    };
    graph.reachability.push(MethodReachability {
        method,
        entrypoints: tests[..5].iter().map(|t| t.method.clone()).collect(),
    });
    (
        vec![changed],
        inventory,
        GoAnalysisResult {
            graph,
            tests: tests.iter().map(|t| t.method.clone()).collect(),
            scope: Some(scope),
        },
    )
}
#[test]
fn partial_graph_suggests_five_but_selects_seven() {
    let (c, i, g) = fixture();
    let d = select_go_tests(&c, &i, Some(&g));
    assert_eq!(
        (
            d.graph_suggested_tests.len(),
            d.selected_tests.len(),
            d.unselected_tests.len()
        ),
        (5, 7, 0)
    );
    assert_eq!(d.selection_mode, GoSelectionMode::ConservativeAll);
    assert!(d.run_all_tests);
}
#[test]
fn graph_failure_keeps_inventory_and_broad_selection() {
    let (c, i, mut g) = fixture();
    g.graph.status = CallGraphStatus::Failed;
    let d = select_go_tests(&c, &i, Some(&g));
    assert!(d.graph_suggested_tests.is_empty());
    assert_eq!(d.selected_tests.len(), 7);
}
#[test]
fn failed_inventory_is_not_an_empty_suite() {
    let (c, mut i, _) = fixture();
    i.tests.clear();
    i.status = GoStageStatus::Failed;
    i.test_inventory_complete = false;
    let d = select_go_tests(&c, &i, None);
    assert!(!d.test_inventory_complete);
    assert!(d.run_all_tests);
    assert!(d.unselected_tests.is_empty());
}
#[test]
fn no_changes_is_explicit_noop() {
    let (_, i, g) = fixture();
    let d = select_go_tests(&[], &i, Some(&g));
    assert_eq!(d.selection_mode, GoSelectionMode::NoChanges);
    assert!(!d.run_all_tests);
    assert!(d.selected_tests.is_empty());
    assert!(!d.test_inventory_complete);
}
#[test]
fn claimed_complete_scope_cannot_enable_exclusion() {
    let (c, mut i, mut g) = fixture();
    i.scope.as_mut().unwrap().dependency_bodies = "complete".into();
    g.scope = i.scope.clone();
    let d = select_go_tests(&c, &i, Some(&g));
    assert_eq!(d.selected_tests.len(), 7);
    assert!(d.unselected_tests.is_empty());
}
#[test]
fn unknown_roots_and_inventory_drift_discard_suggestions() {
    for case in [0, 1, 2] {
        let (c, i, mut g) = fixture();
        match case {
            0 => g.graph.reachability[0].entrypoints[0].member_name = "TestUnknown".into(),
            1 => {
                g.tests.pop();
            }
            _ => g.graph.source_root = "/elsewhere".into(),
        };
        let d = select_go_tests(&c, &i, Some(&g));
        assert!(d.graph_suggested_tests.is_empty(), "case {case}");
        assert_eq!(d.selected_tests.len(), 7);
    }
}
#[test]
fn repeated_targets_and_multiple_reasons_are_deduplicated_without_loss() {
    let (mut c, i, g) = fixture();
    c.push(c[0].clone());
    let d = select_go_tests(&c, &i, Some(&g));
    assert_eq!(d.graph_suggested_tests.len(), 5);
    assert_eq!(d.graph_suggested_tests[0].changed_targets.len(), 1);
    assert_eq!(d.selected_tests[0].reasons.len(), 2);
}
