#![cfg(unix)]
#[path = "../../extractor-runtime/tests/support/go_package_runtime.rs"]
mod support;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::symlink,
    process::{Command, Output},
};
use support::*;

fn run(f: &Fixture, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_go-test-impact"))
        .args([
            "--project-dir",
            f.options.project_dir.to_str().unwrap(),
            "--baseline-revision",
            &f.options.baseline_revision,
            "--go-callgraph-binary",
            f.options.go_callgraph_binary.to_str().unwrap(),
            "--output",
            f.dir.path().join("result.json").to_str().unwrap(),
            "--analysis-timeout-seconds",
            "5",
        ])
        .args(flags)
        .env_remove("GOWORK")
        .env_remove("GOFLAGS")
        .output()
        .unwrap()
}
fn read(f: &Fixture, out: Output) -> Value {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stdout.is_empty());
    serde_json::from_slice(&fs::read(f.dir.path().join("result.json")).unwrap()).unwrap()
}
const PACKAGE_FLAGS: [&str; 4] = [
    "--selection-policy",
    "packages",
    "--test-scope",
    "linked-binary",
];

#[test]
fn opt_in_scope_is_required_and_never_implicitly_accepted() {
    let f = fixture();
    for args in [
        vec!["--selection-policy", "packages"],
        vec!["--test-scope", "linked-binary"],
        vec![
            "--selection-policy",
            "conservative",
            "--test-scope",
            "linked-binary",
        ],
        vec!["--selection-policy", "unknown"],
        vec![
            "--selection-policy",
            "packages",
            "--test-scope",
            "integration",
        ],
    ] {
        let out = run(&f, &args);
        assert!(!out.status.success(), "accepted {args:?}");
        assert!(!f.dir.path().join("calls").exists());
        assert!(!f.dir.path().join("result.json").exists());
    }
}
#[test]
fn default_stays_schema_one_and_scoped_subset_publishes_schema_two() {
    let f = fixture();
    mutate(&f);
    let old = read(&f, run(&f, &[]));
    assert_eq!(old["schema_version"], 1);
    assert!(old.get("requested_policy").is_none());
    assert_eq!(old["selected_tests"].as_array().unwrap().len(), 2);
    assert!(
        !fs::read_to_string(f.dir.path().join("calls"))
            .unwrap()
            .contains("package-inventory")
    );
    let report = read(&f, run(&f, &PACKAGE_FLAGS));
    assert_eq!(report["schema_version"], 2);
    assert_eq!(report["requested_policy"], "packages");
    assert_eq!(report["declared_test_scope"], "linked-binary");
    assert_eq!(report["selection_mode"], "conservative_packages");
    assert_eq!(report["execution_inventory_complete"], true);
    assert_eq!(report["run_all_tests"], false);
    assert_eq!(report["selected_execution_packages"][0]["argument"], "./a");
    assert_eq!(
        report["unselected_execution_packages"][0]["argument"],
        "./b"
    );
    assert_eq!(report["selected_tests"].as_array().unwrap().len(), 1);
}
#[test]
fn no_op_and_unavailable_evidence_are_not_empty_successful_execution_inventories() {
    let f = fixture();
    let noop = read(&f, run(&f, &PACKAGE_FLAGS));
    assert_eq!(noop["schema_version"], 2);
    assert_eq!(noop["selection_mode"], "no_changes");
    assert_eq!(noop["execution_inventory_complete"], false);
    assert_eq!(noop["effective_build_context"], Value::Null);
    assert_eq!(noop["package_evidence"]["status"], "skipped");
    mutate(&f);
    fs::write(f.dir.path().join("packages"), "{}").unwrap();
    let fallback = read(&f, run(&f, &PACKAGE_FLAGS));
    assert_eq!(fallback["run_all_tests"], true);
    assert_eq!(fallback["test_inventory_complete"], true);
    assert_eq!(fallback["execution_inventory_complete"], false);
    assert_eq!(fallback["discovered_execution_packages"], json!([]));
}
#[test]
fn every_structured_blocker_forces_module_execution() {
    for code in [
        "test_main",
        "dynamic_loading",
        "process_execution",
        "unsafe_linkage",
        "cgo_or_assembly",
        "outside_local_replacement",
        "uninspectable_local_sources",
    ] {
        let mut f = fixture();
        mutate(&f);
        f.evidence
            .blockers
            .push(models::go_package_impact::GoPackageBlocker {
                code: code.into(),
                source_path: None,
                diagnostic: "fixture safeguard".into(),
            });
        fs::write(
            f.dir.path().join("packages"),
            serde_json::to_vec(&f.evidence).unwrap(),
        )
        .unwrap();
        let report = read(&f, run(&f, &PACKAGE_FLAGS));
        assert_eq!(report["run_all_tests"], true, "{code}");
        assert_eq!(report["selection_mode"], "conservative_all");
        assert_eq!(report["execution_inventory_complete"], false);
        assert!(
            report["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d.as_str().unwrap().contains(code))
        );
    }
}
#[test]
fn unsafe_execution_directories_cannot_be_published_as_package_instructions() {
    for directory in ["../x", "a/...", "/tmp"] {
        let mut f = fixture();
        mutate(&f);
        f.evidence.execution_packages[0].relative_directory = directory.into();
        fs::write(
            f.dir.path().join("packages"),
            serde_json::to_vec(&f.evidence).unwrap(),
        )
        .unwrap();
        let report = read(&f, run(&f, &PACKAGE_FLAGS));
        assert_eq!(report["run_all_tests"], true);
        assert_eq!(report["execution_inventory_complete"], false);
        assert_eq!(report["selected_execution_packages"], json!([]));
    }
}
#[test]
fn scoped_output_rejects_dirty_input_symlinks_and_failed_publication() {
    let f = fixture();
    let out = f.dir.path().join("result.json");
    fs::write(&out, "previous").unwrap();
    fs::write(f.options.project_dir.join("dirty.go"), "package example").unwrap();
    assert!(!run(&f, &PACKAGE_FLAGS).status.success());
    assert_eq!(fs::read_to_string(out).unwrap(), "previous");
    let f = fixture();
    symlink(
        f.options.project_dir.join("a/a.go"),
        f.dir.path().join("result.json"),
    )
    .unwrap();
    assert!(!run(&f, &PACKAGE_FLAGS).status.success());
    let f = fixture();
    let out = f.dir.path().join("result.json");
    fs::create_dir(&out).unwrap();
    fs::write(out.join("keep"), "previous").unwrap();
    assert!(!run(&f, &PACKAGE_FLAGS).status.success());
    assert_eq!(fs::read_to_string(out.join("keep")).unwrap(), "previous");
}

#[test]
fn eligible_empty_selection_is_an_explicit_skip_not_a_bare_go_test() {
    let mut f = fixture();
    let root = &f.options.project_dir;
    fs::create_dir(root.join("unused")).unwrap();
    fs::write(
        root.join("unused/u.go"),
        "package unused\nfunc Value() int {\n return 1\n}\n",
    )
    .unwrap();
    commit(&f);
    f.options.baseline_revision = git(root, &["rev-parse", "HEAD"]);
    let path = root.join("unused/u.go").to_string_lossy().into_owned();
    let mut i: Value =
        serde_json::from_slice(&fs::read(f.dir.path().join("inventory")).unwrap()).unwrap();
    i["source_files"]
        .as_array_mut()
        .unwrap()
        .push(path.clone().into());
    i["declarations"][0]["method"]["source_path"] = path.clone().into();
    i["declarations"][0]["method"]["declaring_type"] = "example/unused".into();
    fs::write(f.dir.path().join("inventory"), i.to_string()).unwrap();
    let d = fs::read_to_string(f.dir.path().join("describe"))
        .unwrap()
        .replace("a/a.go", "unused/u.go")
        .replace("\"package_name\":\"a\"", "\"package_name\":\"unused\"");
    fs::write(f.dir.path().join("describe"), d).unwrap();
    f.evidence.packages.push(serde_json::from_value(json!({"id":"unused","import_path":"example/unused","imports":[],"provenance":{"kind":"verified_module","directory":root.join("unused"),"module_path":"example"},"source_files":[path],"test_files":[],"for_test":""})).unwrap());
    fs::write(
        f.dir.path().join("packages"),
        serde_json::to_vec(&f.evidence).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("unused/u.go"),
        "package unused\nfunc Value() int {\n return 2\n}\n",
    )
    .unwrap();
    commit(&f);
    let report = read(&f, run(&f, &PACKAGE_FLAGS));
    assert_eq!(report["selection_mode"], "conservative_packages");
    assert_eq!(report["execution_inventory_complete"], true);
    assert_eq!(report["selected_execution_packages"], json!([]));
    assert_eq!(report["run_all_tests"], false);
    assert_eq!(
        report["unselected_execution_packages"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn root_execution_package_uses_literal_dot() {
    let mut f = fixture();
    let root = &f.options.project_dir;
    for (a, b) in [("a/a.go", "a.go"), ("a/a_test.go", "a_test.go")] {
        fs::rename(root.join(a), root.join(b)).unwrap();
    }
    let replacements = [
        (
            root.join("a/a.go").to_string_lossy().into_owned(),
            root.join("a.go").to_string_lossy().into_owned(),
        ),
        (
            root.join("a/a_test.go").to_string_lossy().into_owned(),
            root.join("a_test.go").to_string_lossy().into_owned(),
        ),
        (
            root.join("a").to_string_lossy().into_owned(),
            root.to_string_lossy().into_owned(),
        ),
        ("example/a".into(), "example".into()),
        ("baseline:a/a.go".into(), "baseline:a.go".into()),
        ("candidate:a/a.go".into(), "candidate:a.go".into()),
    ];
    fn rewrite(v: &mut Value, replacements: &[(String, String)]) {
        match v {
            Value::String(s) => {
                for (old, new) in replacements {
                    if s == old {
                        *s = new.clone();
                        break;
                    }
                }
            }
            Value::Array(a) => {
                for v in a {
                    rewrite(v, replacements)
                }
            }
            Value::Object(o) => {
                for v in o.values_mut() {
                    rewrite(v, replacements)
                }
            }
            _ => {}
        }
    }
    for filename in ["inventory", "describe", "packages"] {
        let path = f.dir.path().join(filename);
        let mut v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        rewrite(&mut v, &replacements);
        if filename == "packages" {
            v["execution_packages"][0]["relative_directory"] = ".".into();
        }
        fs::write(path, v.to_string()).unwrap();
    }
    commit(&f);
    f.options.baseline_revision = git(root, &["rev-parse", "HEAD"]);
    fs::write(
        root.join("a.go"),
        "package a\nfunc Value() int {\n return 2\n}\n",
    )
    .unwrap();
    commit(&f);
    let report = read(&f, run(&f, &PACKAGE_FLAGS));
    assert_eq!(report["selection_mode"], "conservative_packages");
    assert_eq!(report["selected_execution_packages"][0]["argument"], ".");
}

#[test]
fn metadata_deadline_publishes_fallback_without_losing_basic_discovery() {
    let f = fixture();
    mutate(&f);
    let script = fs::read_to_string(&f.options.go_callgraph_binary)
        .unwrap()
        .replace(
            &format!("cat '{}/packages'", f.dir.path().display()),
            "sleep 10 & wait",
        );
    fs::write(&f.options.go_callgraph_binary, script).unwrap();
    let report = read(&f, run(&f, &PACKAGE_FLAGS));
    assert_eq!(report["package_evidence"]["status"], "failed");
    assert_eq!(report["test_inventory_complete"], true);
    assert_eq!(report["discovered_tests"].as_array().unwrap().len(), 2);
    assert_eq!(report["execution_inventory_complete"], false);
    assert_eq!(report["run_all_tests"], true);
}
