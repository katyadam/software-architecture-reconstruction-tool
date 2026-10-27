use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn run_generator(args: &[&str]) -> (bool, String, String) {
    let bin = env!("CARGO_BIN_EXE_generate-java-tia-manifest");
    let output = Command::new(bin).args(args).output().unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn setup_git_project(root: &Path) {
    git(root, &["init", "-q"]);
    git(root, &["config", "user.name", "Fixture"]);
    git(root, &["config", "user.email", "fixture@example.invalid"]);
    write(root, "README.md", "fixture");
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "init"]);
}

#[test]
fn generates_manifest_for_single_module_project() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir(&root).unwrap();
    setup_git_project(&root);

    write(
        &root,
        "src/main/java/com/example/Service.java",
        "package com.example; public class Service {}",
    );
    write(
        &root,
        "src/test/java/com/example/ServiceTest.java",
        r#"package com.example;
import org.junit.jupiter.api.Test;
public class ServiceTest {
    @Test public void checksValue() {}
    @Test public void checksEdge() {}
    public void helper() {}
}"#,
    );
    write(&root, "pom.xml", "<project/>");

    let output = dir.path().join("manifest.json");
    let (ok, _stdout, stderr) = run_generator(&[
        "--project-dir",
        root.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ]);
    assert!(ok, "generator failed: {stderr}");
    assert!(stderr.contains("manifest.json"), "expected wrote confirmation in stderr");

    let manifest: Value = serde_json::from_str(&fs::read_to_string(&output).unwrap()).unwrap();
    assert_eq!(manifest["schema_version"], 1);
    assert_eq!(manifest["module_inventory_complete"], true);
    assert_eq!(manifest["build_context"]["tool"], "maven");
    assert_eq!(manifest["build_context"]["java_version"], "17");

    let modules = manifest["modules"].as_array().unwrap();
    assert_eq!(modules.len(), 1);
    let module = &modules[0];
    assert_eq!(module["root"], "");
    assert_eq!(module["test_inventory_complete"], true);

    let tests = module["tests"].as_array().unwrap();
    assert_eq!(tests.len(), 2, "expected 2 @Test methods, got: {tests:?}");
    let method_names: Vec<&str> = tests
        .iter()
        .map(|t| t["method_name"].as_str().unwrap())
        .collect();
    assert!(method_names.contains(&"checksValue"), "missing checksValue");
    assert!(method_names.contains(&"checksEdge"), "missing checksEdge");
    assert!(!method_names.contains(&"helper"), "helper must not be included");

    let class_names: Vec<&str> = tests
        .iter()
        .map(|t| t["class_name"].as_str().unwrap())
        .collect();
    assert!(
        class_names.iter().all(|c| *c == "Lcom/example/ServiceTest"),
        "unexpected class name: {class_names:?}"
    );
}

#[test]
fn generates_manifest_for_multi_module_project() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir(&root).unwrap();
    setup_git_project(&root);

    write(&root, "pom.xml", "<project/>");
    write(&root, "alpha/pom.xml", "<project/>");
    write(
        &root,
        "alpha/src/test/java/pkg/AlphaTest.java",
        r#"package pkg;
import org.junit.jupiter.api.Test;
public class AlphaTest { @Test public void runs() {} }"#,
    );
    write(&root, "beta/pom.xml", "<project/>");
    write(
        &root,
        "beta/src/test/java/pkg/BetaTest.java",
        r#"package pkg;
import org.junit.jupiter.api.Test;
public class BetaTest { @Test public void passes() {} }"#,
    );

    let output = dir.path().join("manifest.json");
    let (ok, _stdout, stderr) = run_generator(&[
        "--project-dir",
        root.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ]);
    assert!(ok, "generator failed: {stderr}");

    let manifest: Value = serde_json::from_str(&fs::read_to_string(&output).unwrap()).unwrap();
    let modules = manifest["modules"].as_array().unwrap();
    assert_eq!(modules.len(), 3, "root + alpha + beta");

    let roots: Vec<&str> = modules
        .iter()
        .map(|m| m["root"].as_str().unwrap())
        .collect();
    assert!(roots.contains(&""), "missing root module");
    assert!(roots.contains(&"alpha"), "missing alpha module");
    assert!(roots.contains(&"beta"), "missing beta module");

    let all_tests: Vec<&str> = modules
        .iter()
        .flat_map(|m| m["tests"].as_array().unwrap())
        .map(|t| t["method_name"].as_str().unwrap())
        .collect();
    assert!(all_tests.contains(&"runs"), "missing AlphaTest.runs");
    assert!(all_tests.contains(&"passes"), "missing BetaTest.passes");
}

#[test]
fn records_source_and_test_roots_as_posix_relative_paths() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir(&root).unwrap();
    setup_git_project(&root);

    write(&root, "pom.xml", "<project/>");
    write(
        &root,
        "src/main/java/com/example/App.java",
        "package com.example; public class App {}",
    );
    write(
        &root,
        "src/test/java/com/example/AppTest.java",
        r#"package com.example;
import org.junit.jupiter.api.Test;
public class AppTest { @Test public void verify() {} }"#,
    );

    let output = dir.path().join("manifest.json");
    let (ok, _stdout, stderr) = run_generator(&[
        "--project-dir",
        root.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ]);
    assert!(ok, "{stderr}");

    let manifest: Value = serde_json::from_str(&fs::read_to_string(&output).unwrap()).unwrap();
    let module = &manifest["modules"][0];

    assert_eq!(module["source_roots"][0], "src/main/java");
    assert_eq!(module["test_source_roots"][0], "src/test/java");

    let test = &module["tests"][0];
    let source_path = test["source_path"].as_str().unwrap();
    assert!(
        !source_path.contains('\\'),
        "source_path must use forward slashes: {source_path}"
    );
    assert!(
        source_path.starts_with("src/test/java"),
        "source_path must be project-relative: {source_path}"
    );
}

#[test]
fn writes_service_config_when_flag_supplied() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir(&root).unwrap();
    setup_git_project(&root);
    write(&root, "pom.xml", "<project/>");

    let manifest_path = dir.path().join("tia-artifacts.json");
    let config_path = dir.path().join("service-config.json");
    let (ok, _stdout, stderr) = run_generator(&[
        "--project-dir",
        root.to_str().unwrap(),
        "--output",
        manifest_path.to_str().unwrap(),
        "--service-config",
        config_path.to_str().unwrap(),
        "--service-name",
        "my-service",
    ]);
    assert!(ok, "{stderr}");
    assert!(config_path.exists(), "service-config.json not created");

    let config: Value =
        serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    let desc = &config["service_descriptions"][0];
    assert_eq!(desc["name"], "my-service");
    assert_eq!(desc["base_dir_path"], ".");

    let artifacts_file = desc["java_analysis"]["artifacts_file"].as_str().unwrap();
    assert!(
        PathBuf::from(artifacts_file).exists(),
        "artifacts_file in service-config must point to an existing file: {artifacts_file}"
    );
}

#[test]
fn respects_custom_build_tool_and_java_version() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir(&root).unwrap();
    setup_git_project(&root);
    write(&root, "pom.xml", "<project/>");

    let output = dir.path().join("manifest.json");
    let (ok, _stdout, stderr) = run_generator(&[
        "--project-dir",
        root.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
        "--build-tool",
        "gradle",
        "--java-version",
        "21",
        "--profiles",
        "ci,integration",
    ]);
    assert!(ok, "{stderr}");

    let manifest: Value = serde_json::from_str(&fs::read_to_string(&output).unwrap()).unwrap();
    let ctx = &manifest["build_context"];
    assert_eq!(ctx["tool"], "gradle");
    assert_eq!(ctx["java_version"], "21");
    let profiles = ctx["requested_profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    assert!(profiles.iter().any(|p| p == "ci"));
    assert!(profiles.iter().any(|p| p == "integration"));
}
