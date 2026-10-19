use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Fixture {
    dir: tempfile::TempDir,
    baseline: String,
    candidate: String,
}
impl Fixture {
    fn root(&self) -> PathBuf {
        self.dir.path().join("project")
    }
    fn config(&self) -> PathBuf {
        self.dir.path().join("config.json")
    }
    fn manifest(&self) -> PathBuf {
        self.dir.path().join("inputs.json")
    }
    fn run(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_test-impact"))
            .args(["--project-dir"])
            .arg(self.root())
            .args(["--baseline-revision", &self.baseline, "--config-file"])
            .arg(self.config())
            .args(["--output"])
            .arg(self.dir.path().join("result.json"))
            .output()
            .unwrap()
    }
    fn report(&self) -> Value {
        let out = self.run();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&fs::read(self.dir.path().join("result.json")).unwrap()).unwrap()
    }
    fn inputs(&self, profile: &str, complete: bool, method: Option<&str>) {
        write(self.dir.path(),"config.json",&json!({"service_descriptions":[{"name":"app","base_dir_path":".","urls":[],"java_analysis":{"maven_profiles":[profile],"artifacts_file":self.manifest()}}]}).to_string());
        let tests=method.map(|name|vec![json!({"module_root":".","class_name":format!("fixture.{name}"),"method_name":"works","descriptor":"()V","callable_signature":format!("{name}.works()V"),"source_path":format!("checks/java/fixture/{name}.java")})]).unwrap_or_default();
        write(self.dir.path(),"inputs.json",&json!({"schema_version":1,"candidate_revision":self.candidate,"build_context":{"tool":"maven","requested_profiles":[profile],"active_profiles":[profile],"java_version":"17"},"module_inventory_complete":true,"modules":[{"root":".","source_roots":[],"test_source_roots":["checks/java"],"application_class_dirs":[],"test_class_dirs":["compiled/tests"],"classpath":[],"dependencies":[],"dependency_inventory_complete":true,"test_inventory_complete":complete,"tests":tests}]}).to_string());
    }
}
fn write(root: &Path, path: &str, body: &str) {
    let p = root.join(path);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, body).unwrap();
}
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.name", "Fixture"]);
    git(&root, &["config", "user.email", "fixture@example.invalid"]);
    for name in ["UnitTest", "IntegrationTest"] {
        write(
            &root,
            &format!("checks/java/fixture/{name}.java"),
            &format!("package fixture; class {name} {{ @Test void works() {{}} }}"),
        );
    }
    write(&root, "application.properties", "value=1");
    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "baseline"]);
    let baseline = git(&root, &["rev-parse", "HEAD"]);
    write(&root, "application.properties", "value=2");
    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "candidate"]);
    let candidate = git(&root, &["rev-parse", "HEAD"]);
    fs::create_dir_all(root.join("compiled/tests")).unwrap();
    Fixture {
        dir,
        baseline,
        candidate,
    }
}

#[test]
fn config_base_dir_requires_config_file() {
    let out = Command::new(env!("CARGO_BIN_EXE_test-impact"))
        .args([
            "--project-dir",
            "/tmp",
            "--baseline-revision",
            "HEAD",
            "--output",
            "/tmp/unused.json",
            "--config-base-dir",
            "/tmp",
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--config-file"));
}

#[test]
fn incomplete_empty_tests_requests_full_run() {
    let f = fixture();
    f.inputs("ci", false, None);
    let result = f.report();
    assert_eq!(result["requires_full_test_run"], true);
    assert_eq!(result["unselected_tests"], json!([]));
    assert_eq!(result["analysis_contexts"].as_array().unwrap().len(), 1);
}

#[test]
fn profile_contexts_do_not_share_inventory() {
    let f = fixture();
    for (profile, class) in [("unit", "UnitTest"), ("integration", "IntegrationTest")] {
        f.inputs(profile, true, Some(class));
        let result = f.report();
        assert_eq!(result["selected_tests"].as_array().unwrap().len(), 1);
        assert_eq!(
            result["selected_tests"][0]["test"]["class_name"],
            format!("Lfixture/{class}")
        );
        assert_eq!(
            result["analysis_contexts"][0]["build_context"]["requested_profiles"],
            json!([profile])
        );
    }
    let mut manifest: Value = serde_json::from_slice(&fs::read(f.manifest()).unwrap()).unwrap();
    manifest["build_context"]["requested_profiles"] = json!(["wrong"]);
    fs::write(f.manifest(), manifest.to_string()).unwrap();
    let out = f.run();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("profile"));
}

#[test]
fn missing_manifest_is_an_input_error_instead_of_default_build() {
    let f = fixture();
    f.inputs("ci", true, None);
    fs::remove_file(f.manifest()).unwrap();
    let out = f.run();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("inputs.json"));
}

#[test]
fn legacy_service_configuration_keeps_automatic_test_discovery() {
    let f = fixture();
    write(&f.root(), "pom.xml", "<project/>");
    write(
        &f.root(),
        "src/test/java/example/LegacyTest.java",
        "package example; class LegacyTest { @Test void legacy() {} }",
    );
    write(
        f.dir.path(),
        "config.json",
        &json!({"service_descriptions":[{"name":"app","base_dir_path":".","urls":[]}]}).to_string(),
    );
    let result = f.report();
    assert!(
        result["selected_tests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["test"]["method_name"] == "legacy")
    );
}
