#![cfg(unix)]
#[path = "support/go_packages.rs"]
mod support;
use extractor_runtime::call_graph::go_native::{GoNativeProvider, validate_package_inventory};
use models::{go_impact::GoStageStatus, go_package_impact::*};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    time::{Duration, Instant},
};

#[test]
fn complete_inventory_owns_all_variants_without_parsing_ids() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let (inventory, evidence) = support::fixture(&root);
    let validated = validate_package_inventory(&root, &inventory, &evidence).unwrap();
    assert_eq!(
        validated.owners()[&root.join("a/a.go").to_string_lossy().into_owned()].len(),
        2
    );
    assert_eq!(
        validated.evidence().execution_packages[0].roots,
        ["binary-opaque"]
    );
}

#[test]
fn malformed_or_incomplete_evidence_never_authorizes_exclusion() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let (inventory, evidence) = support::fixture(&root);
    let json = serde_json::to_value(evidence).unwrap();
    for case in [
        "version",
        "provider",
        "missing_complete",
        "failed",
        "incomplete",
        "duplicate",
        "dangling",
        "missing_root",
        "duplicate_binary",
        "foreign",
        "ownership",
        "missing_file",
        "missing_test_root",
        "omitted_group",
        "directory",
        "toolchain",
        "platform",
        "tags",
        "cgo",
        "arch_feature",
        "missing_arch_feature",
        "blocker",
        "outside_replacement",
        "generated_local",
    ] {
        let mut v = json.clone();
        match case {
            "version" => v["schema_version"] = 2.into(),
            "provider" => v["provider_id"] = "other".into(),
            "missing_complete" => {
                v.as_object_mut().unwrap().remove("complete");
            }
            "failed" => v["status"] = "failed".into(),
            "incomplete" => v["complete"] = false.into(),
            "duplicate" => {
                let n = v["packages"][0].clone();
                v["packages"].as_array_mut().unwrap().push(n);
            }
            "dangling" => v["packages"][0]["imports"] = serde_json::json!(["absent"]),
            "missing_root" => v["execution_packages"][0]["roots"] = serde_json::json!(["absent"]),
            "duplicate_binary" => {
                let n = v["execution_packages"][0].clone();
                v["execution_packages"].as_array_mut().unwrap().push(n);
            }
            "foreign" => v["packages"][0]["source_files"] = serde_json::json!(["/foreign/a.go"]),
            "ownership" => {
                let file = v["packages"][1]["test_files"][0].clone();
                v["packages"][0]["source_files"]
                    .as_array_mut()
                    .unwrap()
                    .push(file);
            }
            "missing_file" => {
                v["packages"][0]["source_files"] = serde_json::json!([]);
                v["packages"][1]["source_files"] = serde_json::json!([root.join("a/a_test.go")]);
            }
            "missing_test_root" => {
                v["execution_packages"][0]["roots"] = serde_json::json!(["production-opaque"])
            }
            "omitted_group" => v["execution_packages"] = serde_json::json!([]),
            "directory" => v["execution_packages"][0]["relative_directory"] = "../a".into(),
            "toolchain" => v["effective_build_context"]["go_version"] = "go1.25".into(),
            "platform" => v["effective_build_context"]["goos"] = "plan9".into(),
            "tags" => v["effective_build_context"]["build_tags"] = "hidden".into(),
            "cgo" => v["effective_build_context"]["cgo_enabled"] = "1".into(),
            "arch_feature" => {
                v["effective_build_context"]["architecture_features"]["GOAMD64"] = "v4".into()
            }
            "missing_arch_feature" => {
                v["effective_build_context"]["architecture_features"] = serde_json::json!({})
            }
            "blocker" => {
                v["blockers"] =
                    serde_json::json!([{"code":"test_main","diagnostic":"unsupported setup"}])
            }
            "outside_replacement" => {
                v["packages"][0]["provenance"]["kind"] = "local_replacement".into();
                v["packages"][0]["provenance"]["directory"] = "/outside".into();
            }
            "generated_local" => {
                v["packages"][0]["provenance"]["kind"] = "generated_test_harness".into()
            }
            _ => unreachable!(),
        }
        if let Ok(e) = serde_json::from_value::<GoPackageInventoryOutcome>(v) {
            assert!(
                validate_package_inventory(&root, &inventory, &e).is_err(),
                "{case} authorized exclusions"
            );
        }
    }
}

#[test]
fn example_only_groups_and_truly_empty_inventory_are_distinct_from_missing_evidence() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let (mut inventory, mut evidence) = support::fixture(&root);
    inventory.tests.clear();
    assert!(validate_package_inventory(&root, &inventory, &evidence).is_ok());
    let saved = evidence.execution_packages.clone();
    evidence.execution_packages.clear();
    assert!(validate_package_inventory(&root, &inventory, &evidence).is_err());
    evidence.execution_packages = saved;
    evidence.packages.clear();
    evidence.execution_packages.clear();
    inventory.source_files.clear();
    assert!(validate_package_inventory(&root, &inventory, &evidence).is_ok());
    evidence.complete = false;
    assert!(validate_package_inventory(&root, &inventory, &evidence).is_err());
}

#[test]
fn exact_arguments_and_symlink_escapes_are_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let (inventory, evidence) = support::fixture(&root);
    for directory in ["/a", "../a", "a/...", "a/../a", "a\\b", "a//b"] {
        let mut e = evidence.clone();
        e.execution_packages[0].relative_directory = directory.into();
        assert!(
            validate_package_inventory(&root, &inventory, &e).is_err(),
            "{directory}"
        );
    }
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("a.go"), "package a").unwrap();
    symlink(outside.path().join("a.go"), root.join("escape.go")).unwrap();
    let mut e = evidence;
    e.packages[0].source_files = vec![root.join("escape.go").to_string_lossy().into_owned()];
    assert!(validate_package_inventory(&root, &inventory, &e).is_err());
}

#[test]
fn legacy_and_timed_out_package_modes_leave_ordinary_inventory_usable() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let (inventory, _) = support::fixture(&root);
    for body in ["echo unsupported >&2; exit 2", "sleep 10 & wait"] {
        let peer = root.join("peer.sh");
        fs::write(&peer, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&peer, fs::Permissions::from_mode(0o755)).unwrap();
        let start = Instant::now();
        let evidence =
            GoNativeProvider::new(&peer, Duration::from_millis(100)).discover_packages(&root);
        assert_eq!(evidence.status, GoStageStatus::Failed);
        assert!(!evidence.complete);
        assert!(!evidence.diagnostics.is_empty());
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(inventory.test_inventory_complete);
        assert_eq!(inventory.tests.len(), 1);
    }
}

#[test]
#[ignore = "requires GO_CALLGRAPH_TEST_BINARY built from go-callgraph"]
fn real_native_package_wire_validates_against_independent_discovery() {
    let binary = std::env::var_os("GO_CALLGRAPH_TEST_BINARY").unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../go-callgraph/testdata/control")
        .canonicalize()
        .unwrap();
    let provider = GoNativeProvider::new(binary, Duration::from_secs(30));
    let inventory = provider.discover(&root);
    let evidence = provider.discover_packages(&root);
    assert_eq!(
        evidence.status,
        GoStageStatus::Ok,
        "{:?}",
        evidence.diagnostics
    );
    let valid = validate_package_inventory(&root, &inventory, &evidence).unwrap();
    assert_eq!(inventory.tests.len(), 7);
    assert!(!valid.evidence().execution_packages.is_empty());
}
