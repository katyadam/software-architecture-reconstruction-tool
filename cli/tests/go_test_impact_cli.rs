#![cfg(unix)]
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, Output},
};
struct Fixture {
    dir: tempfile::TempDir,
    root: PathBuf,
    baseline: String,
    peer: PathBuf,
    out: PathBuf,
}
fn git(root: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .args([
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@e.test",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().into()
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("module");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    git(&root, &["init", "-q"]);
    fs::write(root.join("go.mod"), "module p\n\ngo 1.26.0\n").unwrap();
    fs::write(
        root.join("s.go"),
        "package p\nfunc Changed() int {return 1}\n",
    )
    .unwrap();
    fs::write(root.join("s_test.go"), "package p\nfunc TestX() {}\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "base"]);
    let baseline = git(&root, &["rev-parse", "HEAD"]);
    let scope = json!({"dependency_bodies":"opaque","goos":"linux","goarch":"amd64","go_version":"go1.26","build_tags":"","workspace_mode":"off","limitations":[]});
    let method = |name: &str, file: &str| json!({"language":"go","declaring_type":"p","member_name":name,"descriptor":"func()","source_path":root.join(file),"source_line":2});
    let syntax = json!({"member_name":"Changed","header":"Changed:func() int","start_line":2,"end_line":2,"body_start_line":2,"body_end_line":2});
    let inventory = json!({"schema_version":1,"provider_id":"go-native","status":"ok","source_root":root,"source_files":[root.join("s.go"),root.join("s_test.go")],"tests":[{"method":method("TestX","s_test.go"),"execution_package":"p"}],"declarations":[{"method":method("Changed","s.go"),"syntax":syntax}],"test_inventory_complete":true,"scope":scope,"diagnostics":[]});
    let described = json!({"schema_version":1,"provider_id":"go-native","status":"ok","files":[{"source_path":"baseline:s.go","package_name":"p","declarations":[syntax],"diagnostics":[]},{"source_path":"candidate:s.go","package_name":"p","declarations":[syntax],"diagnostics":[]}],"diagnostics":[]});
    fs::write(dir.path().join("inventory"), inventory.to_string()).unwrap();
    fs::write(dir.path().join("describe"), described.to_string()).unwrap();
    let peer = dir.path().join("peer.sh");
    fs::write(&peer,format!("#!/bin/sh\necho \"$*\" >> '{}/calls'\nif [ \"$1\" = --mode ]; then\n case \"$2\" in\n inventory) cat '{}/inventory';;\n describe) cat >/dev/null; cat '{}/describe';;\n esac\nelse\n echo callgraph_phase=building >&2\n sleep 10 & wait\nfi\n",dir.path().display(),dir.path().display(),dir.path().display())).unwrap();
    fs::set_permissions(&peer, fs::Permissions::from_mode(0o755)).unwrap();
    let out = dir.path().join("result.json");
    Fixture {
        dir,
        root,
        baseline,
        peer,
        out,
    }
}
fn run(f: &Fixture) -> Output {
    Command::new(env!("CARGO_BIN_EXE_go-test-impact"))
        .args([
            "--project-dir",
            f.root.to_str().unwrap(),
            "--baseline-revision",
            &f.baseline,
            "--go-callgraph-binary",
            f.peer.to_str().unwrap(),
            "--output",
            f.out.to_str().unwrap(),
            "--analysis-timeout-seconds",
            "5",
        ])
        .env_remove("GOWORK")
        .env_remove("GOFLAGS")
        .output()
        .unwrap()
}
fn read(f: &Fixture, o: Output) -> Value {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    serde_json::from_slice(&fs::read(&f.out).unwrap()).unwrap()
}
fn commit(f: &Fixture) {
    git(&f.root, &["add", "-A"]);
    git(&f.root, &["commit", "-qm", "candidate"]);
}
#[test]
fn no_changes_invokes_no_native_modes() {
    let f = fixture();
    let v = read(&f, run(&f));
    assert_eq!(v["selection_mode"], "no_changes");
    assert_eq!(v["run_all_tests"], false);
    assert!(!f.dir.path().join("calls").exists());
}
#[test]
fn configuration_change_discovers_without_graph() {
    let f = fixture();
    fs::write(f.root.join("config.yml"), "x: 1").unwrap();
    commit(&f);
    let v = read(&f, run(&f));
    assert_eq!(v["selected_tests"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["analysis"]["status"], "skipped");
    let calls = fs::read_to_string(f.dir.path().join("calls")).unwrap();
    assert_eq!(calls.lines().count(), 1);
    assert!(calls.contains("--mode inventory"));
}
#[test]
fn graph_deadline_retains_inventory() {
    let f = fixture();
    fs::write(
        f.root.join("s.go"),
        "package p\nfunc Changed() int {return 2}\n",
    )
    .unwrap();
    commit(&f);
    let o = run(&f);
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("callgraph_phase=building"),
        "stderr={} report={:?}",
        String::from_utf8_lossy(&o.stderr),
        fs::read_to_string(&f.out)
    );
    let v = read(&f, o);
    assert_eq!(v["analysis"]["status"], "failed");
    assert_eq!(v["selected_tests"].as_array().unwrap().len(), 1);
    assert_eq!(v["unselected_tests"], json!([]));
    assert_eq!(v["run_all_tests"], true);
    assert_eq!(v["test_inventory_complete"], true);
}
#[test]
fn discovery_failure_sets_run_all_instead_of_empty_suite() {
    let f = fixture();
    fs::write(f.root.join("config"), "x").unwrap();
    commit(&f);
    fs::write(f.dir.path().join("inventory"), "not json").unwrap();
    let v = read(&f, run(&f));
    assert_eq!(v["run_all_tests"], true);
    assert_eq!(v["test_inventory_complete"], false);
    assert_eq!(v["discovery"]["status"], "failed");
}
#[test]
fn invalid_input_does_not_replace_previous_report() {
    let f = fixture();
    fs::write(&f.out, "previous").unwrap();
    fs::write(f.root.join("dirty.go"), "package p").unwrap();
    assert!(!run(&f).status.success());
    assert_eq!(fs::read_to_string(&f.out).unwrap(), "previous");
}
#[test]
fn output_symlink_cannot_redirect_into_checkout() {
    let f = fixture();
    symlink(f.root.join("s.go"), &f.out).unwrap();
    assert!(!run(&f).status.success());
    assert_eq!(
        fs::read_to_string(f.root.join("s.go")).unwrap(),
        "package p\nfunc Changed() int {return 1}\n"
    );
}
#[test]
fn ignored_active_go_source_cannot_masquerade_as_committed_head() {
    let f = fixture();
    fs::write(f.root.join(".gitignore"), "ignored.go\n").unwrap();
    commit(&f);
    let p = f.root.join("ignored.go");
    fs::write(&p, "package p; var X=1").unwrap();
    let data = fs::read_to_string(f.dir.path().join("inventory")).unwrap();
    let mut v: Value = serde_json::from_str(&data).unwrap();
    v["source_files"].as_array_mut().unwrap().push(json!(p));
    fs::write(f.dir.path().join("inventory"), v.to_string()).unwrap();
    assert!(!run(&f).status.success());
}
#[test]
fn real_revision_ids_are_reported() {
    let f = fixture();
    fs::write(f.root.join("config"), "x").unwrap();
    commit(&f);
    let v = read(&f, run(&f));
    assert_eq!(v["baseline_revision"], f.baseline);
    assert_eq!(
        v["candidate_revision"],
        git(&f.root, &["rev-parse", "HEAD"])
    );
}
#[test]
fn progress_is_stderr_not_report_text() {
    let f = fixture();
    fs::write(f.root.join("config"), "x").unwrap();
    commit(&f);
    let o = run(&f);
    assert!(o.stdout.is_empty());
    assert!(String::from_utf8_lossy(&o.stderr).contains("go_impact_phase="));
    read(&f, o);
}
#[test]
fn failed_publish_preserves_existing_report() {
    let f = fixture();
    fs::create_dir(&f.out).unwrap();
    fs::write(f.out.join("keep"), "previous").unwrap();
    assert!(!run(&f).status.success());
    assert_eq!(fs::read_to_string(f.out.join("keep")).unwrap(), "previous");
}
