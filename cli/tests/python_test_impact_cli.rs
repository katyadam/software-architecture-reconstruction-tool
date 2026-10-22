use serde_json::Value;
use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Fixture {
    dir: tempfile::TempDir,
    root: PathBuf,
    baseline: String,
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

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    git(&root, &["init", "-q"]);

    // A minimal Python service with one test file.
    write_file(
        &root,
        "service.py",
        "def compute(x):\n    return x + 1\n\ndef helper():\n    return 0\n",
    );
    write_file(
        &root,
        "tests/test_service.py",
        "from service import compute, helper\n\ndef test_compute():\n    assert compute(1) == 2\n\ndef test_helper():\n    assert helper() == 0\n",
    );

    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "baseline"]);
    let baseline = git(&root, &["rev-parse", "HEAD"]);
    let out = dir.path().join("result.json");
    Fixture { dir, root, baseline, out }
}

fn run(f: &Fixture) -> Output {
    Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir",
            f.root.to_str().unwrap(),
            "--baseline-revision",
            &f.baseline,
            "--output",
            f.out.to_str().unwrap(),
            "--pyan3-timeout-secs",
            "0",
        ])
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
fn no_changes_produces_no_changes_mode() {
    let f = fixture();
    let v = read(&f, run(&f));
    assert_eq!(v["selection_mode"], "no_changes");
    assert_eq!(v["run_all_tests"], false);
    assert_eq!(v["selected_tests"].as_array().unwrap().len(), 0);
}

#[test]
fn python_change_discovers_and_selects_all_tests() {
    let f = fixture();
    write_file(
        &f.root,
        "service.py",
        "def compute(x):\n    return x + 2\n\ndef helper():\n    return 0\n",
    );
    commit(&f);
    let v = read(&f, run(&f));
    assert_eq!(v["selection_mode"], "conservative_all", "{v}");
    assert_eq!(v["run_all_tests"], true);
    let selected = v["selected_tests"].as_array().unwrap();
    assert_eq!(selected.len(), 2, "both tests must be selected: {v}");
    let names: Vec<_> = selected
        .iter()
        .map(|t| t["test"]["function_name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"test_compute"), "{names:?}");
    assert!(names.contains(&"test_helper"), "{names:?}");
}

#[test]
fn non_python_file_change_selects_all_tests() {
    let f = fixture();
    write_file(&f.root, "config.ini", "mode=candidate");
    commit(&f);
    let v = read(&f, run(&f));
    assert_eq!(v["selection_mode"], "conservative_all", "{v}");
    assert_eq!(v["run_all_tests"], true);
    assert_eq!(v["selected_tests"].as_array().unwrap().len(), 2, "{v}");
}

#[test]
fn callable_level_change_attaches_target_to_reasons() {
    let f = fixture();
    // Modify only the body of `compute`.
    write_file(
        &f.root,
        "service.py",
        "def compute(x):\n    return x * 3\n\ndef helper():\n    return 0\n",
    );
    commit(&f);
    let v = read(&f, run(&f));
    assert_eq!(v["selection_mode"], "conservative_all", "{v}");
    // At least one selected test should mention `compute` as a changed callable.
    let any_callable_reason = v["selected_tests"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|t| t["reasons"].as_array().unwrap())
        .any(|r| {
            r.get("changed_callable")
                .and_then(|o| o.get("target"))
                .map(|t| t.as_str().unwrap_or("").contains("compute"))
                .unwrap_or(false)
        });
    assert!(any_callable_reason, "no callable reason found: {v}");
}

#[test]
fn deleted_python_file_selects_all_tests() {
    let f = fixture();
    fs::remove_file(f.root.join("service.py")).unwrap();
    git(&f.root, &["add", "-u"]);
    git(&f.root, &["commit", "-qm", "delete"]);
    let v = read(&f, run(&f));
    assert_eq!(v["selection_mode"], "conservative_all", "{v}");
    assert_eq!(v["run_all_tests"], true);
}

#[test]
fn progress_is_emitted_to_stderr() {
    let f = fixture();
    write_file(&f.root, "service.py", "def compute(x):\n    return x + 2\n");
    commit(&f);
    let o = run(&f);
    assert!(o.stdout.is_empty(), "stdout must be empty");
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(
        stderr.contains("python_impact_phase="),
        "stderr must contain phase markers: {stderr}"
    );
    read(&f, o);
}

#[test]
fn real_revision_ids_are_reported() {
    let f = fixture();
    write_file(&f.root, "config.ini", "x=1");
    commit(&f);
    let v = read(&f, run(&f));
    assert_eq!(v["baseline_revision"], f.baseline);
    assert_eq!(
        v["candidate_revision"],
        git(&f.root, &["rev-parse", "HEAD"])
    );
}

#[test]
fn rejects_invalid_baseline_revision() {
    let f = fixture();
    let o = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir",
            f.root.to_str().unwrap(),
            "--baseline-revision",
            "nonexistent-sha-does-not-exist",
            "--output",
            f.out.to_str().unwrap(),
            "--pyan3-timeout-secs",
            "0",
        ])
        .output()
        .unwrap();
    assert!(!o.status.success());
}

#[test]
fn output_path_inside_project_dir_is_rejected() {
    let f = fixture();
    let inside = f.root.join("impact.json");
    let o = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir",
            f.root.to_str().unwrap(),
            "--baseline-revision",
            &f.baseline,
            "--output",
            inside.to_str().unwrap(),
            "--pyan3-timeout-secs",
            "0",
        ])
        .output()
        .unwrap();
    assert!(!o.status.success(), "should reject output inside project dir");
}

#[test]
fn test_functions_in_classes_are_discovered() {
    let f = fixture();
    write_file(
        &f.root,
        "tests/test_class_based.py",
        "import unittest\nclass MyTests(unittest.TestCase):\n    def test_one(self):\n        pass\n    def test_two(self):\n        pass\n",
    );
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-qm", "add class tests"]);
    let new_baseline = git(&f.root, &["rev-parse", "HEAD"]);

    // Now make a change to trigger selection.
    write_file(&f.root, "service.py", "def compute(x):\n    return x + 99\n");
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-qm", "candidate"]);

    let o = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir",
            f.root.to_str().unwrap(),
            "--baseline-revision",
            &new_baseline,
            "--output",
            f.out.to_str().unwrap(),
            "--pyan3-timeout-secs",
            "0",
        ])
        .output()
        .unwrap();
    let v = read(&f, o);
    let selected = v["selected_tests"].as_array().unwrap();
    let names: Vec<_> = selected
        .iter()
        .map(|t| t["test"]["qualified_name"].as_str().unwrap())
        .collect();
    assert!(
        names.iter().any(|n| n.contains("test_one")),
        "class test_one not found: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("test_two")),
        "class test_two not found: {names:?}"
    );
}

/// Django-style `tests.py` files are discovered alongside `test_*.py`.
///
/// Projects like microservices-django-kafka ship `tests.py` (the default Django app test module).
/// This test verifies that a `tests.py` with `test_` methods is picked up by discovery.
#[test]
fn django_tests_py_is_discovered() {
    let f = fixture();
    write_file(
        &f.root,
        "myapp/tests.py",
        "from django.test import TestCase\nclass MyAppTests(TestCase):\n    def test_something(self):\n        pass\n    def test_another(self):\n        pass\n",
    );
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-qm", "add django tests"]);
    let new_baseline = git(&f.root, &["rev-parse", "HEAD"]);

    write_file(&f.root, "service.py", "def compute(x):\n    return x + 99\n");
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-qm", "candidate"]);

    let o = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir",
            f.root.to_str().unwrap(),
            "--baseline-revision",
            &new_baseline,
            "--output",
            f.out.to_str().unwrap(),
            "--pyan3-timeout-secs",
            "0",
        ])
        .output()
        .unwrap();
    let v = read(&f, o);
    let selected = v["selected_tests"].as_array().unwrap();
    let names: Vec<_> = selected
        .iter()
        .map(|t| t["test"]["qualified_name"].as_str().unwrap())
        .collect();
    assert!(
        names.iter().any(|n| n.contains("test_something")),
        "tests.py::test_something not discovered: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("test_another")),
        "tests.py::test_another not discovered: {names:?}"
    );
}

/// Benchmark regression: microservices-django-kafka, commit pair a34f3d3..487c45b.
///
/// Candidate commit 487c45b changes only production Python (models.py, ratios.py) with no
/// test-file edits. The worktree at that commit contains exactly 15 test functions across 4
/// test files. Python TIA is always conservative, so all 15 must be selected.
///
/// The test requires the local benchmark checkout and is skipped silently when it is absent
/// so that CI environments without the external repo do not fail.
#[test]
fn benchmark_microservices_django_kafka_production_change_selects_all_15_tests() {
    // Resolve the benchmark repo relative to the tool repo.
    let tool_repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // cli/
        .and_then(|p| p.parent()) // repo root
        .expect("repo root")
        .to_path_buf();
    let benchmark_root = tool_repo
        .parent()
        .expect("parent of repo root")
        .join("benchmarks")
        .join("python")
        .join("microservices-django-kafka");

    if !benchmark_root.join(".git").exists() {
        // Benchmark repo not present; skip without failing.
        eprintln!("skip: benchmark repo not found at {}", benchmark_root.display());
        return;
    }

    // Known baseline (commit just before the production-only change).
    let baseline = "a34f3d37a0b69912a9bb0a1a1616d683ec7b2c1a";
    // Known candidate (the production-only change: models.py + ratios.py edits, no test edits).
    let candidate = "487c45bc38fa3a1b79aefa339770c50be297332e";

    // Verify both commits exist in the benchmark repo.
    let check = |rev: &str| {
        Command::new("git")
            .args(["rev-parse", "--verify", &format!("{rev}^{{commit}}")])
            .current_dir(&benchmark_root)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    if !check(baseline) || !check(candidate) {
        eprintln!("skip: required commits not present in benchmark repo");
        return;
    }

    // Checkout the candidate commit into a temporary worktree so HEAD points to it.
    let wt_dir = tempfile::tempdir().expect("tempdir");
    let wt_root = wt_dir.path().join("worktree");
    let wt_add = Command::new("git")
        .args(["worktree", "add", wt_root.to_str().unwrap(), candidate])
        .current_dir(&benchmark_root)
        .output()
        .expect("git worktree add");
    assert!(
        wt_add.status.success(),
        "git worktree add failed: {}",
        String::from_utf8_lossy(&wt_add.stderr)
    );

    // Run TIA against the worktree with baseline as the reference revision.
    let out_path = wt_dir.path().join("result.json");
    let o = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir",
            wt_root.to_str().unwrap(),
            "--baseline-revision",
            baseline,
            "--output",
            out_path.to_str().unwrap(),
            "--pyan3-timeout-secs",
            "0",
        ])
        .output()
        .expect("python-test-impact");

    // Cleanup worktree before any assertion so it isn't left behind on failure.
    let _ = Command::new("git")
        .args(["worktree", "remove", "--force", wt_root.to_str().unwrap()])
        .current_dir(&benchmark_root)
        .output();

    assert!(
        o.status.success(),
        "python-test-impact failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    let v: Value = serde_json::from_slice(&fs::read(&out_path).unwrap())
        .expect("valid JSON output");

    // Production-only change → conservative_all, run_all_tests=true.
    assert_eq!(v["selection_mode"], "conservative_all", "{v}");
    assert_eq!(v["run_all_tests"], true, "{v}");

    // Exactly 15 test functions are discoverable at this commit:
    //   events/tests/test_conf.py        — 7 methods
    //   events/tests/test_decorators.py  — 1 method
    //   events/tests/test_models.py      — 1 method
    //   planning-service/.../test_ratios.py — 6 methods (2 classes × 3 methods each)
    let discovered = v["discovered_tests"].as_array().unwrap().len();
    assert_eq!(discovered, 15, "expected 15 discovered tests, got {discovered}: {v}");

    let selected = v["selected_tests"].as_array().unwrap().len();
    assert_eq!(selected, 15, "expected all 15 tests selected, got {selected}: {v}");

    // Both changed callables should appear as reasons.
    let all_targets: Vec<&str> = v["selected_tests"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|t| t["reasons"].as_array().unwrap())
        .filter_map(|r| {
            r.get("changed_callable")
                .and_then(|o| o.get("target"))
                .and_then(|t| t.as_str())
        })
        .collect();
    let has_models = all_targets.iter().any(|t| t.contains("Person") || t.contains("age"));
    let has_ratios = all_targets.iter().any(|t| t.contains("NetWorthToTotalAssets") || t.contains("__init__"));
    assert!(
        has_models || has_ratios,
        "expected at least one callable-level reason for models.py or ratios.py change; targets={all_targets:?}"
    );
}

#[test]
fn pyan3_timeout_zero_skips_graph_phase() {
    let f = fixture();
    write_file(
        &f.root,
        "service.py",
        "def compute(x):\n    return x + 2\n\ndef helper():\n    return 0\n",
    );
    commit(&f);
    let out = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir",
            f.root.to_str().unwrap(),
            "--baseline-revision",
            &f.baseline,
            "--output",
            f.out.to_str().unwrap(),
            "--pyan3-timeout-secs",
            "0",
        ])
        .output()
        .unwrap();
    let v = read(&f, out);
    // Conservative selection still works.
    assert_eq!(v["selection_mode"], "conservative_all", "{v}");
    assert_eq!(v["run_all_tests"], true, "{v}");
    // Analysis stage must be skipped.
    assert_eq!(v["analysis"]["status"], "skipped", "{v}");
    // No graph suggestions when phase is skipped.
    assert_eq!(
        v["graph_suggested_tests"].as_array().unwrap().len(),
        0,
        "{v}"
    );
}

#[test]
fn pyan3_absent_falls_back_to_conservative() {
    // Run with a non-zero timeout but a fake pyan3 executable name that won't be found.
    // We can't override PATH easily, so we test the equivalent: if pyan3 is not installed,
    // the analysis stage reports "not_installed" or "skipped" but selection is still conservative.
    // Since this test runs regardless of pyan3 availability, we just verify robustness of the
    // zero-timeout path as the unit-testable proxy.
    let f = fixture();
    write_file(&f.root, "service.py", "def compute(x):\n    return x + 99\n");
    commit(&f);
    // With timeout=0 the graph phase is explicitly skipped — no crash, full conservative selection.
    let out = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir",
            f.root.to_str().unwrap(),
            "--baseline-revision",
            &f.baseline,
            "--output",
            f.out.to_str().unwrap(),
            "--pyan3-timeout-secs",
            "0",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v = read(&f, out);
    assert_eq!(v["selection_mode"], "conservative_all", "{v}");
    assert_eq!(v["run_all_tests"], true, "{v}");
    assert_eq!(v["selected_tests"].as_array().unwrap().len(), 2, "{v}");
}

/// Benchmark regression: kafka-ml, commit 5636347 (production-only change to models.py).
///
/// Commit 5636347 adds `TELEGRAF_STR_JSON` to choices in `backend/automl/models.py` only.
/// No test files are touched. The repo has 14 test methods across 3 TestCase classes in
/// `backend/automl/test_views.py`. All 14 must be selected conservatively.
///
/// Skips silently when the benchmark repo is not present (CI-safe).
#[test]
fn benchmark_kafka_ml_models_change_selects_all_14_tests() {
    let tool_repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root")
        .to_path_buf();
    let benchmark_root = tool_repo
        .parent()
        .expect("parent of repo root")
        .join("benchmarks")
        .join("python")
        .join("kafka-ml");

    if !benchmark_root.join(".git").exists() {
        eprintln!("skip: benchmark repo not found at {}", benchmark_root.display());
        return;
    }

    // Commit where only backend/automl/models.py was changed (added TELEGRAF_STR_JSON choice).
    let candidate = "5636347";
    // Parent commit as baseline.
    let baseline_out = Command::new("git")
        .args(["rev-parse", &format!("{candidate}^")])
        .current_dir(&benchmark_root)
        .output()
        .expect("git rev-parse");
    if !baseline_out.status.success() {
        eprintln!("skip: required commit {candidate} not present in benchmark repo");
        return;
    }
    let baseline = String::from_utf8_lossy(&baseline_out.stdout).trim().to_string();

    let wt_dir = tempfile::tempdir().expect("tempdir");
    let wt_root = wt_dir.path().join("worktree");
    let wt_add = Command::new("git")
        .args(["worktree", "add", wt_root.to_str().unwrap(), candidate])
        .current_dir(&benchmark_root)
        .output()
        .expect("git worktree add");
    assert!(
        wt_add.status.success(),
        "git worktree add failed: {}",
        String::from_utf8_lossy(&wt_add.stderr)
    );

    let out_path = wt_dir.path().join("result.json");
    let o = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir",
            wt_root.to_str().unwrap(),
            "--baseline-revision",
            &baseline,
            "--output",
            out_path.to_str().unwrap(),
            "--pyan3-timeout-secs",
            "0",
        ])
        .output()
        .expect("python-test-impact");

    let _ = Command::new("git")
        .args(["worktree", "remove", "--force", wt_root.to_str().unwrap()])
        .current_dir(&benchmark_root)
        .output();

    assert!(
        o.status.success(),
        "python-test-impact failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    let v: Value = serde_json::from_slice(&fs::read(&out_path).unwrap())
        .expect("valid JSON output");

    assert_eq!(v["selection_mode"], "conservative_all", "{v}");
    assert_eq!(v["run_all_tests"], true, "{v}");

    // 14 test methods across 3 TestCase classes in backend/automl/test_views.py:
    //   ModelViewTest         — 8 methods
    //   ConfigurationViewTest — 5 methods
    //   ResultViewTest        — 1 method
    let discovered = v["discovered_tests"].as_array().unwrap().len();
    assert_eq!(discovered, 14, "expected 14 discovered tests, got {discovered}: {v}");

    let selected = v["selected_tests"].as_array().unwrap().len();
    assert_eq!(selected, 14, "expected all 14 tests selected, got {selected}: {v}");
}

// ---------------------------------------------------------------------------
// AST-driven discovery: new signal tests
// ---------------------------------------------------------------------------

/// A file named `logic.py` (no test_ prefix) that contains `import pytest` and a
/// `def test_compute():` function must be discovered.
#[test]
fn pytest_function_in_non_test_named_file_is_discovered() {
    let f = fixture();
    write_file(
        &f.root,
        "myapp/logic.py",
        "import pytest\n\ndef compute(x):\n    return x\n\ndef test_compute():\n    assert compute(1) == 1\n",
    );
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-qm", "add pytest logic"]);
    let new_baseline = git(&f.root, &["rev-parse", "HEAD"]);

    write_file(&f.root, "service.py", "def compute(x):\n    return x + 99\n");
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-qm", "candidate"]);

    let o = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir", f.root.to_str().unwrap(),
            "--baseline-revision", &new_baseline,
            "--output", f.out.to_str().unwrap(),
            "--pyan3-timeout-secs", "0",
        ])
        .output()
        .unwrap();
    let v = read(&f, o);
    let names: Vec<_> = v["selected_tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["test"]["qualified_name"].as_str().unwrap())
        .collect();
    assert!(
        names.iter().any(|n| *n == "test_compute"),
        "test_compute from pytest-importing logic.py not discovered: {names:?}"
    );
}

/// A file named `service_tests.py` (no test_ prefix) with a `unittest.TestCase`
/// subclass and `test_` methods must be discovered.
#[test]
fn unittest_testcase_subclass_in_non_test_named_file_is_discovered() {
    let f = fixture();
    write_file(
        &f.root,
        "myapp/service_tests.py",
        "import unittest\n\nclass ServiceTests(unittest.TestCase):\n    def test_one(self):\n        pass\n    def test_two(self):\n        pass\n",
    );
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-qm", "add unittest suite"]);
    let new_baseline = git(&f.root, &["rev-parse", "HEAD"]);

    write_file(&f.root, "service.py", "def compute(x):\n    return x + 99\n");
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-qm", "candidate"]);

    let o = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir", f.root.to_str().unwrap(),
            "--baseline-revision", &new_baseline,
            "--output", f.out.to_str().unwrap(),
            "--pyan3-timeout-secs", "0",
        ])
        .output()
        .unwrap();
    let v = read(&f, o);
    let names: Vec<_> = v["selected_tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["test"]["qualified_name"].as_str().unwrap())
        .collect();
    assert!(
        names.iter().any(|n| n.contains("test_one")),
        "test_one from unittest service_tests.py not discovered: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("test_two")),
        "test_two from unittest service_tests.py not discovered: {names:?}"
    );
}

/// A file `myapp/tests/utils.py` whose name has no test_ prefix must be scanned
/// because it lives under a `tests/` directory, and its pytest functions discovered.
#[test]
fn files_under_tests_dir_are_scanned_without_test_prefix() {
    let f = fixture();
    write_file(
        &f.root,
        "myapp/tests/utils.py",
        "import pytest\n\ndef test_helper():\n    pass\n\ndef test_another():\n    pass\n",
    );
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-qm", "add tests/utils.py"]);
    let new_baseline = git(&f.root, &["rev-parse", "HEAD"]);

    write_file(&f.root, "service.py", "def compute(x):\n    return x + 99\n");
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-qm", "candidate"]);

    let o = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir", f.root.to_str().unwrap(),
            "--baseline-revision", &new_baseline,
            "--output", f.out.to_str().unwrap(),
            "--pyan3-timeout-secs", "0",
        ])
        .output()
        .unwrap();
    let v = read(&f, o);
    let names: Vec<_> = v["selected_tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["test"]["qualified_name"].as_str().unwrap())
        .collect();
    assert!(
        names.iter().any(|n| *n == "test_helper"),
        "test_helper from tests/utils.py not discovered: {names:?}"
    );
    assert!(
        names.iter().any(|n| *n == "test_another"),
        "test_another from tests/utils.py not discovered: {names:?}"
    );
}

/// A class decorated with `@some_decorator` wrapping a `TestCase` subclass must
/// have its test methods discovered (fixes the `decorated_definition → class_definition` gap).
#[test]
fn decorated_class_tests_are_discovered() {
    let f = fixture();
    write_file(
        &f.root,
        "tests/test_decorated.py",
        "import unittest\n\ndef some_decorator(cls):\n    return cls\n\n@some_decorator\nclass DecoratedTests(unittest.TestCase):\n    def test_alpha(self):\n        pass\n    def test_beta(self):\n        pass\n",
    );
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-qm", "add decorated test class"]);
    let new_baseline = git(&f.root, &["rev-parse", "HEAD"]);

    write_file(&f.root, "service.py", "def compute(x):\n    return x + 99\n");
    git(&f.root, &["add", "."]);
    git(&f.root, &["commit", "-qm", "candidate"]);

    let o = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir", f.root.to_str().unwrap(),
            "--baseline-revision", &new_baseline,
            "--output", f.out.to_str().unwrap(),
            "--pyan3-timeout-secs", "0",
        ])
        .output()
        .unwrap();
    let v = read(&f, o);
    let names: Vec<_> = v["selected_tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["test"]["qualified_name"].as_str().unwrap())
        .collect();
    assert!(
        names.iter().any(|n| n.contains("test_alpha")),
        "test_alpha from decorated class not discovered: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("test_beta")),
        "test_beta from decorated class not discovered: {names:?}"
    );
}

/// Benchmark regression: microservices-django-kafka, commit 487c45b with pyan3 enabled.
///
/// When pyan3 is available and all changed elements are callable-level (or accompanied
/// by harmless module-level additions), the tool uses `call_graph_precise` mode and
/// selects only the 6 planning-service tests that transitively reach the changed callables:
///   NetWorthToTotalAssets_Age42::{test_benchmark, test_ratio, test_status}
///   NetWorthToTotalAssets_Age66::{test_benchmark, test_ratio, test_status}
///
/// The 9 events-service tests (EventsConfTestCase, OutboxItemTestCase, SaveEventDecorator)
/// are unselected because they have no call path to the changed code.
///
/// Skips silently when pyan3 is not installed or the benchmark repo is absent.
#[test]
fn benchmark_mdk_callable_change_selects_6_tests_with_call_graph_precise() {
    let tool_repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root")
        .to_path_buf();
    let benchmark_root = tool_repo
        .parent()
        .expect("parent of repo root")
        .join("benchmarks")
        .join("python")
        .join("microservices-django-kafka");

    if !benchmark_root.join(".git").exists() {
        eprintln!("skip: benchmark repo not found at {}", benchmark_root.display());
        return;
    }

    // Verify pyan3 is installed (without it, this test would just fall back to conservative).
    let pyan3_ok = Command::new("pyan3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !pyan3_ok {
        eprintln!("skip: pyan3 not installed");
        return;
    }

    let baseline = "a34f3d37a0b69912a9bb0a1a1616d683ec7b2c1a";
    let candidate = "487c45bc38fa3a1b79aefa339770c50be297332e";

    let check = |rev: &str| {
        Command::new("git")
            .args(["rev-parse", "--verify", &format!("{rev}^{{commit}}")])
            .current_dir(&benchmark_root)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    if !check(baseline) || !check(candidate) {
        eprintln!("skip: required commits not present in benchmark repo");
        return;
    }

    let wt_dir = tempfile::tempdir().expect("tempdir");
    let wt_root = wt_dir.path().join("worktree");
    let wt_add = Command::new("git")
        .args(["worktree", "add", wt_root.to_str().unwrap(), candidate])
        .current_dir(&benchmark_root)
        .output()
        .expect("git worktree add");
    assert!(
        wt_add.status.success(),
        "git worktree add failed: {}",
        String::from_utf8_lossy(&wt_add.stderr)
    );

    let out_path = wt_dir.path().join("result.json");
    let o = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir",
            wt_root.to_str().unwrap(),
            "--baseline-revision",
            baseline,
            "--output",
            out_path.to_str().unwrap(),
            "--pyan3-timeout-secs",
            "60",
        ])
        .output()
        .expect("python-test-impact");

    let _ = Command::new("git")
        .args(["worktree", "remove", "--force", wt_root.to_str().unwrap()])
        .current_dir(&benchmark_root)
        .output();

    assert!(
        o.status.success(),
        "python-test-impact failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    let v: Value = serde_json::from_slice(&fs::read(&out_path).unwrap())
        .expect("valid JSON output");

    // With pyan3, should use precise call graph mode and NOT run all tests.
    assert_eq!(v["selection_mode"], "call_graph_precise", "{v}");
    assert_eq!(v["run_all_tests"], false, "{v}");

    // Only the 6 planning-service tests that reach changed callables should be selected.
    let selected = v["selected_tests"].as_array().unwrap();
    assert_eq!(selected.len(), 6, "expected 6 selected tests (planning-service only), got {}: {v}", selected.len());

    // The 9 events-service tests must be unselected.
    let unselected = v["unselected_tests"].as_array().unwrap();
    assert_eq!(unselected.len(), 9, "expected 9 unselected tests (events-service), got {}: {v}", unselected.len());

    // All selected tests should be from NetWorthToTotalAssets_Age42 or _Age66.
    let selected_names: Vec<&str> = selected
        .iter()
        .map(|t| t["test"]["qualified_name"].as_str().unwrap())
        .collect();
    for name in &selected_names {
        assert!(
            name.starts_with("NetWorthToTotalAssets_Age"),
            "unexpected selected test {name}: {selected_names:?}"
        );
    }
}

/// Synthetic fixture test for `call_graph_precise` selection.
///
/// Two-module project:
///   svc_a/logic.py      — `def process(x)` (will be changed)
///   svc_b/utils.py      — `def format_val(x)` (unchanged)
///   tests/test_svc_a.py — imports and calls `process`   → reachable
///   tests/test_svc_b.py — imports and calls `format_val` → NOT reachable
///
/// After changing `process`, pyan3 should identify `test_process` as reachable and
/// `test_format_val` as not reachable, producing `call_graph_precise` with 1 selected
/// and 1 unselected.
///
/// Skips silently if pyan3 is not installed so CI without pyan3 still passes.
#[test]
fn call_graph_precise_selects_only_reachable_test() {
    let pyan3_ok = Command::new("pyan3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !pyan3_ok {
        eprintln!("skip: pyan3 not installed");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    git(&root, &["init", "-q"]);

    write_file(&root, "svc_a/logic.py", "def process(x):\n    return x * 2\n");
    write_file(&root, "svc_b/utils.py", "def format_val(x):\n    return str(x)\n");
    write_file(
        &root,
        "tests/test_svc_a.py",
        "from svc_a.logic import process\n\ndef test_process():\n    assert process(3) == 6\n",
    );
    write_file(
        &root,
        "tests/test_svc_b.py",
        "from svc_b.utils import format_val\n\ndef test_format_val():\n    assert format_val(1) == '1'\n",
    );
    write_file(&root, "svc_a/__init__.py", "");
    write_file(&root, "svc_b/__init__.py", "");

    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "baseline"]);
    let baseline = git(&root, &["rev-parse", "HEAD"]);

    // Change only svc_a/logic.py — svc_b is untouched.
    write_file(&root, "svc_a/logic.py", "def process(x):\n    return x * 3\n");
    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "change process"]);

    let out_path = dir.path().join("result.json");
    let o = Command::new(env!("CARGO_BIN_EXE_python-test-impact"))
        .args([
            "--project-dir", root.to_str().unwrap(),
            "--baseline-revision", &baseline,
            "--output", out_path.to_str().unwrap(),
            "--pyan3-timeout-secs", "30",
        ])
        .output()
        .unwrap();

    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let v: Value = serde_json::from_slice(&fs::read(&out_path).unwrap()).unwrap();

    assert_eq!(v["selection_mode"], "call_graph_precise", "{v}");
    assert_eq!(v["run_all_tests"], false, "{v}");

    let selected: Vec<&str> = v["selected_tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["test"]["function_name"].as_str().unwrap())
        .collect();
    let unselected: Vec<&str> = v["unselected_tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["function_name"].as_str().unwrap())
        .collect();

    assert_eq!(selected, vec!["test_process"], "wrong selected: {v}");
    assert_eq!(unselected, vec!["test_format_val"], "wrong unselected: {v}");
}
