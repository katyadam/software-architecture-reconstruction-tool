use models::{CodeElementsAggregate, ConfigurationData};
use serde_json::json;
use std::{fs, path::Path, process::Command, time::Duration};

fn write(root: &Path, path: &str, body: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

#[test]
fn plain_reconstruction_does_not_require_optional_build_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    write(
        &root,
        "source/java/Service.java",
        "class Service { void work() {} }",
    );
    let config = dir.path().join("config.json");
    fs::write(&config,json!({"service_descriptions":[{"name":"app","base_dir_path":root,"urls":[],"java_analysis":{"artifacts_file":"absent-inputs.json"}}]}).to_string()).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(["--project-dir"])
        .arg(&root)
        .args(["--config-file"])
        .arg(&config)
        .args(["--output-dir"])
        .arg(dir.path().join("plain"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(["--project-dir"])
        .arg(&root)
        .args(["--config-file"])
        .arg(&config)
        .args(["--output-dir"])
        .arg(dir.path().join("enriched"))
        .args(["--wala-adapter-jar", "/missing/adapter.jar"])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "explicit manifest must be validated when enrichment is requested"
    );
}

#[test]
fn reconstruction_enrichment_honors_generated_source_roots_and_prepared_classes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    write(
        &root,
        "target/generated-sources/fixture/Service.java",
        "package fixture; public class Service { public static void first() { second(); } public static void second() {} }",
    );
    fs::create_dir_all(root.join("compiled/main")).unwrap();
    let manifest = json!({"schema_version":1,"candidate_revision":"0123456789012345678901234567890123456789","build_context":{"tool":"maven","requested_profiles":[],"active_profiles":[],"java_version":"17"},"module_inventory_complete":true,"modules":[{"root":".","source_roots":["target/generated-sources"],"test_source_roots":[],"application_class_dirs":["compiled/main"],"test_class_dirs":[],"classpath":[],"dependencies":[],"dependency_inventory_complete":true,"test_inventory_complete":true,"tests":[]}]});
    write(&root, "inputs.json", &manifest.to_string());
    let config:ConfigurationData=serde_json::from_value(json!({"service_descriptions":[{"name":"app","base_dir_path":root,"urls":[],"java_analysis":{"artifacts_file":"inputs.json"}}]})).unwrap();
    let inputs = extractor_runtime::java_analysis::load_java_analysis_inputs(
        &root,
        &root,
        &config,
        "0123456789012345678901234567890123456789",
    )
    .unwrap();
    let method = |name: &str| json!({"language":"java","declaring_type":"Lfixture/Service","member_name":name,"descriptor":"()V","source_path":null,"source_line":null});
    let graph = json!({"schema_version":2,"status":"ok","provider_id":"wala-java","source_root":root,"algorithm":"cha","diagnostics":[],"reachability":[],"edges":[{"caller":method("first"),"callee":method("second"),"provider_id":"wala-java","algorithm":"cha","confidence":null}]});
    let java = dir.path().join("Main.java");
    fs::write(&java,format!("public class Main {{public static void main(String[] args) {{if (!java.util.Arrays.asList(args).contains(\"--classes-dir\")) System.exit(8);System.out.print({});}}}}",serde_json::to_string(&graph.to_string()).unwrap())).unwrap();
    assert!(Command::new("javac").arg(&java).status().unwrap().success());
    let jar = dir.path().join("adapter.jar");
    assert!(
        Command::new("jar")
            .arg("cfe")
            .arg(&jar)
            .arg("Main")
            .arg("-C")
            .arg(dir.path())
            .arg("Main.class")
            .status()
            .unwrap()
            .success()
    );
    let enriched = cli::enrich_with_wala_inputs(
        CodeElementsAggregate::default(),
        &root,
        &config,
        Some(&jar),
        Duration::from_secs(10),
        Some(&inputs),
    );
    assert_eq!(
        enriched.callables.len(),
        2,
        "declared generated sources must be collected even though ordinary traversal skips target"
    );
    assert_eq!(
        enriched.resolved_call_edges.len(),
        1,
        "prepared classes must reach the provider and match the generated source"
    );
}
