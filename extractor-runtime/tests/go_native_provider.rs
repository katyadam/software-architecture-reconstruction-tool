//! The transport tests use small executable protocol peers, not a mocked graph
//! library. The ignored control separately exercises the real Go executable.
#![cfg(unix)]

use extractor_runtime::call_graph::{
    CallGraphProvider, ProviderRegistry, go_native::GoNativeProvider,
};
use models::call_graph::{CallGraphRequest, CallGraphStatus, Language};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    time::{Duration, Instant},
};
use tempfile::tempdir;

fn response(root: &Path) -> Value {
    let method = |name: &str, file: &str| {
        json!({
            "language":"go", "declaring_type":"example.test/control", "member_name":name,
            "descriptor":"func()", "source_path":root.join(file), "source_line":1
        })
    };
    fs::write(
        root.join("service.go"),
        "package control; func Changed() {}\n",
    )
    .unwrap();
    fs::write(root.join("service_test.go"), "package control\n").unwrap();
    let test = method("TestDirect", "service_test.go");
    json!({
        "schema_version":2, "status":"ok", "provider_id":"go-native", "algorithm":"cha",
        "source_root":root, "diagnostics":[], "edges":[],
        "reachability":[{"method":method("Changed", "service.go"), "entrypoints":[test.clone()]}],
        "tests":[test], "scope":{
            "dependency_bodies":"opaque", "goos":"linux", "goarch":"amd64",
            "go_version":"go1.26.0", "build_tags":"control", "workspace_mode":"off",
            "limitations":["external-library callbacks", "reflection and plugins"]
        }
    })
}

fn peer(root: &Path, body: &str) -> std::path::PathBuf {
    let path = root.join("native peer.sh");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn emitting_peer(root: &Path, value: &Value) -> std::path::PathBuf {
    let data = root.join("response.json");
    fs::write(&data, serde_json::to_vec(value).unwrap()).unwrap();
    // The generated temporary path contains no quote; arguments are still passed
    // to the provider as OS arguments, never composed into a shell command.
    peer(root, &format!("cat '{}'", data.display()))
}

#[test]
fn preserves_native_tests_scope_and_target_arguments_without_certifying_exclusion() {
    let dir = tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let value = response(&root);
    fs::write(
        root.join("response.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    let executable = peer(
        &root,
        &format!(
            "printf '%s\\n' \"$@\" > '{}'; echo callgraph_phase=loading >&2; cat '{}'",
            root.join("args.txt").display(),
            root.join("response.json").display()
        ),
    );
    let provider =
        GoNativeProvider::new(executable, Duration::from_secs(3)).with_build_tags("control");
    let result = provider.analyze_targets(
        &root,
        &[
            "example.test/control#Changed".into(),
            "example.test/control#Changed".into(),
        ],
    );
    assert_eq!(
        result.graph.status,
        CallGraphStatus::Ok,
        "{:?}",
        result.graph.diagnostics
    );
    assert_eq!(result.tests[0].member_name, "TestDirect");
    assert_eq!(result.tests[0].descriptor, "func()");
    assert_eq!(result.scope.as_ref().unwrap().dependency_bodies, "opaque");
    assert_eq!(result.scope.as_ref().unwrap().limitations.len(), 2);
    assert!(result.requires_conservative_fallback());
    let args = fs::read_to_string(root.join("args.txt")).unwrap();
    assert!(args.contains("--source-dir\n"));
    assert!(args.contains("--target\nexample.test/control#Changed\n"));
    assert!(args.contains("--tags\ncontrol\n"));
    assert!(args.contains("--timeout\n3s\n"));
    assert_eq!(
        args.lines().filter(|arg| *arg == "--target").count(),
        1,
        "repeated changed declarations must not duplicate protocol targets"
    );

    // The old, scope-less generic contract must not turn partial reachability
    // into an apparently complete successful graph.
    let generic = result.into_conservative_outcome();
    assert_eq!(generic.status, CallGraphStatus::Unsupported);
    assert!(generic.reachability.is_empty());
    assert!(generic.diagnostics.iter().any(|d| d.contains("opaque")));
}

#[test]
fn registry_path_fails_closed_and_keeps_java_requests_unsupported() {
    let dir = tempdir().unwrap();
    let provider = GoNativeProvider::new(dir.path().join("missing"), Duration::from_secs(1));
    assert_eq!(provider.language(), Language::Go);
    let registry = ProviderRegistry::new(vec![Box::new(provider)]);
    let outcome = registry.analyze(&CallGraphRequest::new(
        dir.path().to_string_lossy(),
        Language::Go,
    ));
    assert_eq!(outcome.status, CallGraphStatus::Unsupported);
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.contains("explicit targets"))
    );
    let java = registry.analyze(&CallGraphRequest::new("/java", Language::Java));
    assert_eq!(java.status, CallGraphStatus::Unsupported);
}

#[test]
fn rejects_untrusted_or_incomplete_protocol_instead_of_empty_success() {
    let dir = tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    for mutation in [
        "schema",
        "provider",
        "algorithm",
        "root",
        "scope",
        "tests",
        "language",
        "entrypoint",
        "target",
        "empty",
        "foreign_path",
        "tags",
    ] {
        let mut value = response(&root);
        match mutation {
            "schema" => value["schema_version"] = json!(99),
            "provider" => value["provider_id"] = json!("wala-java"),
            "algorithm" => value["algorithm"] = json!("rta"),
            "root" => value["source_root"] = json!("/elsewhere"),
            "scope" => {
                value.as_object_mut().unwrap().remove("scope");
            }
            "tests" => {
                value.as_object_mut().unwrap().remove("tests");
            }
            "language" => value["tests"][0]["language"] = json!("java"),
            "entrypoint" => {
                value["reachability"][0]["entrypoints"][0]["member_name"] =
                    json!("TestNotDiscovered")
            }
            "target" => value["reachability"][0]["method"]["member_name"] = json!("Wrong"),
            "empty" => value["reachability"] = json!([]),
            "foreign_path" => value["tests"][0]["source_path"] = json!("/foreign/test.go"),
            "tags" => value["scope"]["build_tags"] = json!("unexpected"),
            _ => unreachable!(),
        }
        let executable = emitting_peer(&root, &value);
        let result = GoNativeProvider::new(executable, Duration::from_secs(2))
            .with_build_tags("control")
            .analyze_targets(&root, &["example.test/control#Changed".into()]);
        assert_eq!(
            result.graph.status,
            CallGraphStatus::Failed,
            "accepted {mutation}"
        );
        assert!(result.graph.reachability.is_empty(), "retained {mutation}");
        assert!(result.requires_conservative_fallback());
    }
}

#[test]
fn retains_structured_failure_diagnostics_even_with_nonzero_exit() {
    let dir = tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let mut value = response(&root);
    value["status"] = json!("failed");
    value["diagnostics"] = json!(["TestMain setup is unsupported"]);
    value["reachability"] = json!([]);
    let data = root.join("response.json");
    fs::write(&data, serde_json::to_vec(&value).unwrap()).unwrap();
    let executable = peer(&root, &format!("cat '{}'; exit 1", data.display()));
    let result = GoNativeProvider::new(executable, Duration::from_secs(2))
        .with_build_tags("control")
        .analyze_targets(&root, &["example.test/control#Changed".into()]);
    assert_eq!(result.graph.status, CallGraphStatus::Failed);
    assert!(
        result
            .graph
            .diagnostics
            .iter()
            .any(|d| d.contains("TestMain"))
    );
}

#[test]
fn handles_spawn_failure_invalid_json_crash_and_timeout_with_progress() {
    let dir = tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let missing = GoNativeProvider::new(root.join("absent"), Duration::from_secs(1))
        .analyze_targets(&root, &["example.test/control#Changed".into()]);
    assert_eq!(missing.graph.status, CallGraphStatus::Failed);
    for body in [
        "echo not-json",
        "echo failure-detail >&2; exit 1",
        "echo callgraph_phase=building >&2; sleep 10 & wait",
    ] {
        let executable = peer(&root, body);
        let started = Instant::now();
        // This tests a deadline, not shell startup speed. Allow cold/loaded
        // process startup while remaining well below the 10-second child.
        let result = GoNativeProvider::new(executable, Duration::from_secs(1))
            .analyze_targets(&root, &["example.test/control#Changed".into()]);
        assert_eq!(result.graph.status, CallGraphStatus::Failed);
        assert!(!result.graph.diagnostics.is_empty());
        assert!(result.graph.reachability.is_empty());
        if body.contains("sleep") {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "descendant inherited pipes blocked timeout"
            );
            assert!(
                result
                    .graph
                    .diagnostics
                    .iter()
                    .any(|d| d.contains("timed out") && d.contains("callgraph_phase=building")),
                "missing deadline telemetry: {:?}",
                result.graph.diagnostics
            );
        }
    }
}

#[test]
#[ignore = "Builds separately; run scripts/test-go-native-adapter.sh"]
fn real_go_executable_preserves_seven_tests_and_five_reachable_roots() {
    let binary =
        std::env::var_os("GO_CALLGRAPH_TEST_BINARY").expect("script supplies native binary");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../go-callgraph/testdata/control")
        .canonicalize()
        .unwrap();
    let result = GoNativeProvider::new(binary, Duration::from_secs(30)).analyze_targets(
        &root,
        &[
            "example.com/tia#Changed".into(),
            "example.com/tia#Changed".into(),
            "example.com/tia#NeverCalled".into(),
        ],
    );
    assert_eq!(
        result.graph.status,
        CallGraphStatus::Ok,
        "{:?}",
        result.graph.diagnostics
    );
    assert_eq!(result.tests.len(), 7);
    assert_eq!(result.graph.reachability[0].entrypoints.len(), 5);
    assert_eq!(result.graph.reachability[1].entrypoints.len(), 0);
    assert!(result.requires_conservative_fallback());
    assert_eq!(
        result.into_conservative_outcome().status,
        CallGraphStatus::Unsupported
    );
}
