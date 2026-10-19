use extractor_runtime::java_analysis::load_java_analysis_inputs;
use models::ConfigurationData;
use serde_json::{Value, json};
use std::{fs, path::Path};
use tempfile::TempDir;

const SHA: &str = "0123456789012345678901234567890123456789";

fn manifest() -> Value {
    json!({"schema_version":1,"candidate_revision":SHA,
        "build_context":{"tool":"maven","requested_profiles":["ci"],"active_profiles":["ci"],"java_version":"17"},
        "module_inventory_complete":true,"modules":[{
            "root":".","source_roots":["source/java"],"test_source_roots":["checks/java"],
            "application_class_dirs":["target/classes"],"test_class_dirs":["target/test-classes"],
            "classpath":[],"dependencies":[],"dependency_inventory_complete":true,
            "test_inventory_complete":true,"tests":[]}]})
}

fn write(root: &Path, path: &str, body: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn setup() -> (TempDir, ConfigurationData) {
    let dir = tempfile::tempdir().unwrap();
    for path in [
        "train-ticket/orders/source/java",
        "train-ticket/orders/checks/java",
        "train-ticket/orders/target/classes",
        "train-ticket/orders/target/test-classes",
    ] {
        fs::create_dir_all(dir.path().join(path)).unwrap();
    }
    write(
        dir.path(),
        "train-ticket/orders/target/inputs.json",
        &manifest().to_string(),
    );
    let config = serde_json::from_value(json!({"service_descriptions":[{
        "name":"orders","base_dir_path":"train-ticket/orders","urls":[],
        "java_analysis":{"maven_profiles":["ci"],"artifacts_file":"target/inputs.json"}}]}))
    .unwrap();
    (dir, config)
}

fn run(
    dir: &TempDir,
    config: &ConfigurationData,
) -> Result<
    extractor_runtime::java_analysis::ConfiguredJavaInputs,
    extractor_runtime::java_analysis::JavaInputsError,
> {
    load_java_analysis_inputs(&dir.path().join("train-ticket"), dir.path(), config, SHA)
}

#[test]
fn train_ticket_paths_and_complete_empty_inventory_are_explicit() {
    let (dir, config) = setup();
    let inputs = run(&dir, &config).unwrap();
    let root = dir
        .path()
        .join("train-ticket/orders")
        .canonicalize()
        .unwrap();
    assert!(inputs.modules.contains_key(&root));
    assert_eq!(
        inputs.modules[&root].source_roots,
        [root.join("source/java")]
    );
    assert_eq!(inputs.contexts[0].build_context.requested_profiles, ["ci"]);
    assert_eq!(inputs.contexts[0].manifest_digest.len(), 64);
    assert!(!inputs.requires_full_test_run());
}

#[test]
fn mismatched_versions_revisions_profiles_and_missing_inputs_are_rejected() {
    let (dir, config) = setup();
    for (field, value) in [
        ("schema_version", json!(2)),
        ("candidate_revision", json!("f".repeat(40))),
    ] {
        let mut m = manifest();
        m[field] = value;
        write(
            dir.path(),
            "train-ticket/orders/target/inputs.json",
            &m.to_string(),
        );
        assert!(run(&dir, &config).is_err(), "{field}");
    }
    let mut m = manifest();
    m["build_context"]["requested_profiles"] = json!(["other"]);
    write(
        dir.path(),
        "train-ticket/orders/target/inputs.json",
        &m.to_string(),
    );
    assert!(run(&dir, &config).is_err());
    fs::remove_file(dir.path().join("train-ticket/orders/target/inputs.json")).unwrap();
    assert!(run(&dir, &config).is_err());
}

#[test]
fn incomplete_empty_inventory_requires_full_run() {
    let (dir, config) = setup();
    for flag in ["test_inventory_complete", "dependency_inventory_complete"] {
        let mut m = manifest();
        m["modules"][0][flag] = json!(false);
        write(
            dir.path(),
            "train-ticket/orders/target/inputs.json",
            &m.to_string(),
        );
        assert!(run(&dir, &config).unwrap().requires_full_test_run());
    }
    let mut m = manifest();
    m["module_inventory_complete"] = json!(false);
    write(
        dir.path(),
        "train-ticket/orders/target/inputs.json",
        &m.to_string(),
    );
    assert!(run(&dir, &config).unwrap().requires_full_test_run());
}

#[test]
fn explicit_profiles_require_manifest_but_empty_settings_keep_automatic_inputs() {
    let (dir, mut config) = setup();
    let settings = config.service_descriptions[0]
        .java_analysis
        .as_mut()
        .unwrap();
    settings.artifacts_file = None;
    assert!(run(&dir, &config).is_err());
    config.service_descriptions[0]
        .java_analysis
        .as_mut()
        .unwrap()
        .maven_profiles
        .clear();
    let inputs = run(&dir, &config).unwrap();
    assert!(inputs.modules.is_empty());
    assert_eq!(inputs.services.len(), 1);
}

#[test]
fn parent_relative_shared_modules_are_allowed_only_inside_checkout() {
    let (dir, config) = setup();
    fs::create_dir_all(dir.path().join("train-ticket/shared")).unwrap();
    let mut m = manifest();
    let mut shared = m["modules"][0].clone();
    shared["root"] = json!("../shared");
    for key in [
        "source_roots",
        "test_source_roots",
        "application_class_dirs",
        "test_class_dirs",
    ] {
        shared[key] = json!([]);
    }
    m["modules"][0]["dependencies"] = json!(["../shared"]);
    m["modules"].as_array_mut().unwrap().push(shared);
    write(
        dir.path(),
        "train-ticket/orders/target/inputs.json",
        &m.to_string(),
    );
    assert_eq!(run(&dir, &config).unwrap().modules.len(), 2);
    m["modules"][1]["root"] = json!("../../");
    write(
        dir.path(),
        "train-ticket/orders/target/inputs.json",
        &m.to_string(),
    );
    assert!(run(&dir, &config).is_err());
}

#[test]
fn relative_manifest_escape_is_rejected_but_absolute_ci_manifest_is_allowed() {
    let (dir, mut config) = setup();
    let external = dir.path().join("inputs.json");
    fs::write(&external, manifest().to_string()).unwrap();
    config.service_descriptions[0]
        .java_analysis
        .as_mut()
        .unwrap()
        .artifacts_file = Some("../../inputs.json".into());
    assert!(run(&dir, &config).is_err());
    config.service_descriptions[0]
        .java_analysis
        .as_mut()
        .unwrap()
        .artifacts_file = Some(external.to_string_lossy().into());
    assert!(run(&dir, &config).is_ok());
}

#[cfg(unix)]
#[test]
fn symlink_source_escape_is_rejected() {
    let (dir, config) = setup();
    std::os::unix::fs::symlink(
        dir.path(),
        dir.path().join("train-ticket/orders/source/outside"),
    )
    .unwrap();
    let mut m = manifest();
    m["modules"][0]["source_roots"] = json!(["source/outside"]);
    write(
        dir.path(),
        "train-ticket/orders/target/inputs.json",
        &m.to_string(),
    );
    assert!(run(&dir, &config).is_err());
}

#[test]
fn outside_services_and_duplicate_or_overlapping_roots_are_not_silently_analyzed() {
    let (dir, mut config) = setup();
    let mut service = config.service_descriptions[0].clone();
    service.name = "other".into();
    service.base_dir_path = ".".into();
    service.java_analysis = None;
    config.service_descriptions.push(service);
    assert_eq!(run(&dir, &config).unwrap().services.len(), 1);
    config.service_descriptions[1].java_analysis = Some(Default::default());
    assert!(run(&dir, &config).is_err());
    config.service_descriptions[1] = config.service_descriptions[0].clone();
    assert!(run(&dir, &config).is_err());
    config.service_descriptions[1].base_dir_path = "train-ticket/orders/source".into();
    config.service_descriptions[1].java_analysis = None;
    assert!(run(&dir, &config).is_err());
}

#[test]
fn malformed_or_duplicate_test_identity_is_rejected() {
    let (dir, config) = setup();
    write(
        dir.path(),
        "train-ticket/orders/checks/java/OrderTest.java",
        "class OrderTest {} ",
    );
    let mut m = manifest();
    let test = json!({"module_root":".","class_name":"OrderTest","method_name":"works",
        "descriptor":"()V","callable_signature":"class:OrderTest/void works()",
        "source_path":"checks/java/OrderTest.java"});
    m["modules"][0]["tests"] = json!([test.clone(), test]);
    write(
        dir.path(),
        "train-ticket/orders/target/inputs.json",
        &m.to_string(),
    );
    assert!(run(&dir, &config).is_err());
    m["modules"][0]["tests"].as_array_mut().unwrap().pop();
    m["modules"][0]["tests"][0]["source_path"] = json!("source/java/Other.java");
    write(
        dir.path(),
        "train-ticket/orders/target/inputs.json",
        &m.to_string(),
    );
    assert!(run(&dir, &config).is_err());
}

#[test]
fn conflicting_shared_module_contexts_are_errors() {
    let (dir, mut config) = setup();
    fs::create_dir_all(dir.path().join("train-ticket/shared")).unwrap();
    fs::create_dir_all(dir.path().join("train-ticket/other")).unwrap();
    let mut m = manifest();
    let mut module = m["modules"][0].clone();
    module["root"] = json!("../shared");
    for key in [
        "source_roots",
        "test_source_roots",
        "application_class_dirs",
        "test_class_dirs",
    ] {
        module[key] = json!([]);
    }
    m["modules"] = json!([module]);
    write(
        dir.path(),
        "train-ticket/orders/target/inputs.json",
        &m.to_string(),
    );
    m["build_context"]["active_profiles"] = json!(["ci", "different"]);
    write(dir.path(), "train-ticket/other/inputs.json", &m.to_string());
    let mut other = config.service_descriptions[0].clone();
    other.name = "other".into();
    other.base_dir_path = "train-ticket/other".into();
    other.java_analysis.as_mut().unwrap().artifacts_file = Some("inputs.json".into());
    config.service_descriptions.push(other);
    assert!(run(&dir, &config).is_err());
}
