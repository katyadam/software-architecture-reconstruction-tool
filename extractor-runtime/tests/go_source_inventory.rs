#![cfg(unix)]
use extractor_runtime::call_graph::go_native::GoNativeProvider;
use models::go_impact::{GoSourceInput, GoStageStatus};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    time::{Duration, Instant},
};
fn peer(root: &Path, body: &str) -> std::path::PathBuf {
    let p = root.join("peer.sh");
    fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    p
}
#[test]
fn description_mode_handles_large_stdin_without_deadlock() {
    let d = tempfile::tempdir().unwrap();
    let binary = peer(d.path(), "sleep 10 & wait");
    let provider = GoNativeProvider::new(binary, Duration::from_secs(1));
    let start = Instant::now();
    let result = provider.describe_sources(&[GoSourceInput {
        source_path: "old:s.go".into(),
        source: " ".repeat(200_000),
    }]);
    assert_eq!(result.status, GoStageStatus::Failed);
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(!result.diagnostics.is_empty());
}
#[test]
fn inventory_validation_rejects_missing_completeness_foreign_paths_and_duplicate_tests() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().canonicalize().unwrap();
    let base = serde_json::json!({"schema_version":1,"provider_id":"go-native","status":"ok","source_root":root,"source_files":[],"tests":[],"declarations":[],"test_inventory_complete":true,"scope":{"dependency_bodies":"opaque","goos":"linux","goarch":"amd64","go_version":"go1.26","build_tags":"","workspace_mode":"off","limitations":[]},"diagnostics":[]});
    for case in ["missing", "foreign", "duplicate"] {
        let mut v = base.clone();
        match case {
            "missing" => {
                v.as_object_mut().unwrap().remove("test_inventory_complete");
            }
            "foreign" => v["source_files"] = serde_json::json!(["/foreign/x.go"]),
            _ => {
                let m = serde_json::json!({"method":{"language":"go","declaring_type":"p","member_name":"TestX","descriptor":"func()","source_path":"/foreign/x_test.go","source_line":1},"execution_package":"p"});
                v["tests"] = serde_json::json!([m, m]);
            }
        };
        fs::write(root.join("reply"), v.to_string()).unwrap();
        let binary = peer(&root, &format!("cat '{}'", root.join("reply").display()));
        let out = GoNativeProvider::new(binary, Duration::from_secs(2)).discover(&root);
        assert_eq!(out.status, GoStageStatus::Failed, "{case}");
        assert!(!out.test_inventory_complete);
    }
}
#[test]
#[ignore = "run scripts/test-go-native-adapter.sh"]
fn inventory_does_not_require_graph_targets() {
    let binary = std::env::var_os("GO_CALLGRAPH_TEST_BINARY").unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../go-callgraph/testdata/control")
        .canonicalize()
        .unwrap();
    let p = GoNativeProvider::new(binary, Duration::from_secs(30));
    let out = p.discover(&root);
    assert_eq!(out.status, GoStageStatus::Ok, "{:?}", out.diagnostics);
    assert_eq!(out.tests.len(), 7);
    assert!(out.test_inventory_complete);
    let described = p.describe_sources(&[GoSourceInput {
        source_path: "baseline:s.go".into(),
        source: "package p; func F() { }".into(),
    }]);
    assert_eq!(described.status, GoStageStatus::Ok);
    assert_eq!(described.files[0].declarations[0].member_name, "F");
    let graph = p.analyze_targets(&root, &["example.com/tia#Changed".into()]);
    assert!(
        graph
            .tests
            .iter()
            .all(|m| out.tests.iter().any(|t| &t.method == m))
    );
}
