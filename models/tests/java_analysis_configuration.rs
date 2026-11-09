use models::{ConfigurationData, TestImpactResult, configuration::ServiceDescription};
use serde_json::json;

#[test]
fn legacy_service_json_round_trips_without_java_analysis() {
    let value = json!({"name":"orders","base_dir_path":"orders","urls":[]});
    let service: ServiceDescription = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(service).unwrap(), value);
}

#[test]
fn java_analysis_settings_are_preserved_and_default_profiles_are_empty() {
    let config: ConfigurationData = serde_json::from_value(json!({
        "service_descriptions":[{"name":"orders","base_dir_path":"orders","urls":[],
            "java_analysis":{"artifacts_file":"target/inputs.json"}}]
    }))
    .unwrap();
    let value = serde_json::to_value(config).unwrap();
    assert_eq!(
        value["service_descriptions"][0]["java_analysis"]["artifacts_file"],
        "target/inputs.json"
    );
    assert_eq!(
        value["service_descriptions"][0]["java_analysis"]["maven_profiles"],
        json!([])
    );
}

#[test]
fn existing_configuration_files_still_deserialize() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../config/configurations");
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|s| s == "json") {
            serde_json::from_slice::<ConfigurationData>(&std::fs::read(&path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
    }
}

#[test]
fn manifest_requires_explicit_completeness() {
    let value = json!({"schema_version":1,"candidate_revision":"abc",
        "build_context":{"tool":"maven","requested_profiles":[],"active_profiles":[],"java_version":"17"},
        "modules":[]});
    assert!(serde_json::from_value::<models::java_analysis::JavaArtifactManifest>(value).is_err());
}

#[test]
fn legacy_result_deserializes_without_new_fields() {
    let legacy = json!({"baseline_revision":"old","candidate_revision":"new",
        "changed_elements":[],"selected_tests":[],"unselected_tests":[],"diagnostics":[]});
    let result: TestImpactResult = serde_json::from_value(legacy.clone()).unwrap();
    assert!(result.analysis_contexts.is_empty());
    assert!(!result.requires_full_test_run);
    assert_eq!(serde_json::to_value(result).unwrap(), legacy);
}
