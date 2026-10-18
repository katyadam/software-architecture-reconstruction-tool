#[path = "support/go_packages.rs"]
mod support;
use extractor_runtime::{
    call_graph::go_native::{GoAnalysisResult, validate_package_inventory},
    impact::go_package_selection::select_go_packages,
};
use models::{
    call_graph::{CallGraphOutcome, CallGraphRequest, Language, MethodReachability},
    go_impact::*,
    go_package_impact::*,
};
use std::{fs, path::Path};

fn module_node(
    root: &Path,
    id: &str,
    dir: &str,
    files: Vec<String>,
    tests: Vec<String>,
    for_test: &str,
    imports: Vec<String>,
) -> GoPackageNode {
    serde_json::from_value(serde_json::json!({"id":id,"import_path":format!("example/{dir}"),"imports":imports,"provenance":{"kind":"verified_module","directory":root.join(dir),"module_path":"example"},"source_files":files,"test_files":tests,"for_test":for_test})).unwrap()
}
fn fixture(root: &Path) -> (GoInventoryOutcome, GoPackageInventoryOutcome) {
    let (mut i, mut e) = support::fixture(root);
    fs::create_dir_all(root.join("helper")).unwrap();
    fs::write(
        root.join("helper/h.go"),
        "package helper; func Value() int { return 1 }",
    )
    .unwrap();
    let helper = root.join("helper/h.go").to_string_lossy().into_owned();
    i.source_files.push(helper.clone());
    e.packages.push(module_node(
        root,
        "helper",
        "helper",
        vec![helper],
        vec![],
        "",
        vec![],
    ));
    for node in &mut e.packages {
        if node.import_path == "example/a" {
            node.imports.push("helper".into());
        }
    }
    for (dir, imports) in [("b", vec![]), ("c", vec!["external-bridge".into()])] {
        fs::create_dir_all(root.join(dir)).unwrap();
        fs::write(
            root.join(dir).join("s_test.go"),
            format!("package {dir}; import \"testing\"; func TestValue(t *testing.T) {{}}"),
        )
        .unwrap();
        let source = root
            .join(dir)
            .join("s_test.go")
            .to_string_lossy()
            .into_owned();
        i.source_files.push(source.clone());
        let mut test = i.tests[0].clone();
        test.method.declaring_type = format!("example/{dir}");
        test.execution_package = format!("example/{dir}");
        test.method.source_path = Some(source.clone());
        i.tests.push(test);
        e.packages.push(module_node(
            root,
            dir,
            dir,
            vec![source.clone()],
            vec![source],
            &format!("example/{dir}"),
            imports,
        ));
        e.execution_packages.push(GoExecutionPackage {
            import_path: format!("example/{dir}"),
            relative_directory: dir.into(),
            roots: vec![dir.into()],
        });
    }
    e.packages.push(serde_json::from_value(serde_json::json!({"id":"external-bridge","import_path":"external/bridge","imports":["helper"],"provenance":{"kind":"module_cache","directory":root.parent().unwrap(),"module_path":"external","module_version":"v1.0.0"},"source_files":[],"test_files":[],"for_test":""})).unwrap());
    fs::write(
        root.join("a/external_test.go"),
        "package a_test; import \"testing\"; func TestSibling(t *testing.T) {}",
    )
    .unwrap();
    let source = root
        .join("a/external_test.go")
        .to_string_lossy()
        .into_owned();
    i.source_files.push(source.clone());
    let mut sibling = i.tests[0].clone();
    sibling.method.declaring_type = "example/a_test".into();
    sibling.method.member_name = "TestSibling".into();
    sibling.method.source_path = Some(source.clone());
    i.tests.push(sibling);
    let mut node = module_node(
        root,
        "external-sibling",
        "a",
        vec![source.clone()],
        vec![source],
        "example/a",
        vec!["binary-opaque".into()],
    );
    node.import_path = "example/a_test".into();
    e.packages.push(node);
    e.execution_packages[0]
        .roots
        .push("external-sibling".into());
    (i, e)
}
fn change(path: &str) -> Vec<GoChangedElement> {
    vec![GoChangedElement {
        source_path: path.into(),
        old_path: Some(path.into()),
        kind: GoChangeKind::Callable,
        target: Some("example/helper#Value".into()),
        diagnostics: vec![],
    }]
}

#[test]
fn whole_binary_closure_keeps_siblings_and_traverses_external_intermediaries() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let (i, e) = fixture(&root);
    let v = validate_package_inventory(&root, &i, &e).unwrap();
    let d = select_go_packages(&change("helper/h.go"), &i, &v, None);
    assert_eq!(
        d.selected_execution_packages
            .iter()
            .map(|p| p.package.import_path.as_str())
            .collect::<Vec<_>>(),
        ["example/a", "example/c"]
    );
    assert_eq!(
        d.unselected_execution_packages[0].package.import_path,
        "example/b"
    );
    assert_eq!(d.tests.selected_tests.len(), 3);
    assert_eq!(d.tests.unselected_tests.len(), 1);
    assert!(!d.tests.run_all_tests);
    assert!(d.execution_inventory_complete);
    assert_eq!(
        d.tests.selection_mode,
        GoSelectionMode::ConservativePackages
    );
    assert!(d.tests.graph_suggested_tests.is_empty());
}
#[test]
fn production_multiowner_and_test_only_helper_changes_keep_only_their_binary() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let (i, e) = fixture(&root);
    let v = validate_package_inventory(&root, &i, &e).unwrap();
    for file in ["a/a.go", "a/a_test.go", "a/external_test.go"] {
        let d = select_go_packages(&change(file), &i, &v, None);
        assert_eq!(d.selected_execution_packages.len(), 1);
        assert_eq!(
            d.selected_execution_packages[0].package.import_path,
            "example/a"
        );
        assert_eq!(d.tests.selected_tests.len(), 2);
    }
}
#[test]
fn all_none_and_example_only_binaries_have_honest_execution_counts() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let (mut i, mut e) = fixture(&root);
    i.tests.retain(|t| t.execution_package != "example/c");
    let v = validate_package_inventory(&root, &i, &e).unwrap();
    let d = select_go_packages(&change("helper/h.go"), &i, &v, None);
    assert_eq!(d.selected_execution_packages.len(), 2);
    let all = [change("helper/h.go"), change("b/s_test.go")].concat();
    let d = select_go_packages(&all, &i, &v, None);
    assert!(d.tests.run_all_tests);
    assert_eq!(d.selected_execution_packages.len(), 3);
    fs::create_dir_all(root.join("unused")).unwrap();
    fs::write(root.join("unused/u.go"), "package unused; func Value() {}").unwrap();
    let file = root.join("unused/u.go").to_string_lossy().into_owned();
    i.source_files.push(file.clone());
    e.packages.push(module_node(
        &root,
        "unused",
        "unused",
        vec![file],
        vec![],
        "",
        vec![],
    ));
    let v = validate_package_inventory(&root, &i, &e).unwrap();
    let d = select_go_packages(&change("unused/u.go"), &i, &v, None);
    assert!(d.selected_execution_packages.is_empty());
    assert_eq!(d.unselected_execution_packages.len(), 3);
    assert!(!d.tests.run_all_tests);
    assert_eq!(
        d.tests.selection_mode,
        GoSelectionMode::ConservativePackages
    );
}
#[test]
fn valid_positive_contradiction_forces_module_fallback_but_invalid_graph_does_not() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let (i, e) = fixture(&root);
    let v = validate_package_inventory(&root, &i, &e).unwrap();
    let changes = change("helper/h.go");
    let mut graph = CallGraphOutcome::ok(
        CallGraphRequest::new(&i.source_root, Language::Go),
        "go-native",
        "cha",
    );
    graph.schema_version = 2;
    let mut method = i.tests[0].method.clone();
    method.declaring_type = "example/helper".into();
    method.member_name = "Value".into();
    method.source_path = Some(root.join("helper/h.go").to_string_lossy().into_owned());
    graph.reachability.push(MethodReachability {
        method,
        entrypoints: vec![i.tests[1].method.clone()],
    });
    let mut g = GoAnalysisResult {
        graph,
        tests: i.tests.iter().map(|t| t.method.clone()).collect(),
        scope: i.scope.clone(),
    };
    let d = select_go_packages(&changes, &i, &v, Some(&g));
    assert_eq!(d.tests.selection_mode, GoSelectionMode::ConservativeAll);
    assert!(d.tests.run_all_tests);
    assert_eq!(d.selected_execution_packages.len(), 3);
    assert!(d.unselected_execution_packages.is_empty());
    g.graph.source_root = "/wrong".into();
    let d = select_go_packages(&changes, &i, &v, Some(&g));
    assert_eq!(
        d.tests.selection_mode,
        GoSelectionMode::ConservativePackages
    );
    assert_eq!(d.unselected_execution_packages.len(), 1);
    g.graph.source_root = i.source_root.clone();
    g.graph.status = models::call_graph::CallGraphStatus::Failed;
    let d = select_go_packages(&changes, &i, &v, Some(&g));
    assert_eq!(d.unselected_execution_packages.len(), 1);
    assert!(d.tests.graph_suggested_tests.is_empty());
}
#[test]
fn broad_or_unowned_changes_cannot_authorize_a_subset() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let (i, e) = fixture(&root);
    let v = validate_package_inventory(&root, &i, &e).unwrap();
    for mut changes in [change("missing.go"), change("a/a.go")] {
        if changes[0].source_path == "a/a.go" {
            changes[0].kind = GoChangeKind::Module;
        }
        let d = select_go_packages(&changes, &i, &v, None);
        assert!(d.tests.run_all_tests);
        assert!(d.unselected_execution_packages.is_empty());
        assert_eq!(d.tests.selection_mode, GoSelectionMode::ConservativeAll);
    }
}

#[test]
fn validated_evidence_cannot_be_reused_with_a_different_inventory() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let (mut i, e) = fixture(&root);
    let v = validate_package_inventory(&root, &i, &e).unwrap();
    i.tests[0].execution_package = "example/another".into();
    let d = select_go_packages(&change("helper/h.go"), &i, &v, None);
    assert_eq!(d.tests.selection_mode, GoSelectionMode::ConservativeAll);
    assert!(d.tests.run_all_tests);
    assert!(d.unselected_execution_packages.is_empty());
}
