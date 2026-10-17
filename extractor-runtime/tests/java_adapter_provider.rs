use extractor_runtime::call_graph::{CallGraphProvider, java_adapter::JavaAdapterProvider};
use models::{
    JavaTestCase,
    call_graph::{CallGraphStatus, Language},
};
use std::{fs, path::Path, process::Command, time::Duration};
use tempfile::tempdir;

#[test]
fn forwards_algorithm_entrypoints_and_targets_to_a_named_adapter() {
    let directory = tempdir().expect("temporary directory");
    let adapter = build_adapter(
        directory.path(),
        r#"System.out.print("{\"schema_version\":2,\"status\":\"ok\",\"provider_id\":\"sootup-java\",\"source_root\":\"/service\",\"algorithm\":\"cha\",\"diagnostics\":[\"ARGS=" + String.join("|", args) + "\"],\"reachability\":[],\"edges\":[]}");"#,
    );
    let provider = JavaAdapterProvider::new("sootup-java", adapter, Duration::from_secs(5), "cha");
    let tests = vec![JavaTestCase {
        module_root: "module".into(),
        class_name: "Lexample/ServiceTest".into(),
        method_name: "coversChanged".into(),
        descriptor: "()V".into(),
        callable_signature: "ServiceTest.coversChanged()V".into(),
        source_path: "src/test/java/example/ServiceTest.java".into(),
    }];

    let outcome = provider.analyze_test_roots(
        Path::new("/service"),
        &tests,
        &["Lexample/Service#changed()V".into()],
    );

    assert_eq!(outcome.status, CallGraphStatus::Ok);
    assert_eq!(outcome.provider_id, "sootup-java");
    assert_eq!(
        outcome.diagnostics,
        vec![
            "ARGS=--algorithm|cha|--source-dir|/service|--classes-dir|/classes|--test-classes-dir|/test-classes|--classpath|/dependency.jar|--entrypoint|Lexample/ServiceTest#coversChanged()V|--target|Lexample/Service#changed()V"
        ],
    );
}

#[test]
fn identifies_the_provider_when_adapter_output_is_malformed() {
    let directory = tempdir().expect("temporary directory");
    let adapter = build_adapter(directory.path(), "System.out.print(\"not-json\");");
    let provider = JavaAdapterProvider::new("sootup-java", adapter, Duration::from_secs(5), "cha");

    let outcome = provider.analyze(&models::call_graph::CallGraphRequest::new(
        "/service",
        Language::Java,
    ));

    assert_eq!(outcome.status, CallGraphStatus::Failed);
    assert_eq!(outcome.provider_id, "sootup-java");
    assert_eq!(
        outcome.diagnostics,
        vec!["sootup-java produced invalid call-graph JSON"]
    );
}

fn build_adapter(directory: &Path, body: &str) -> std::path::PathBuf {
    let source = directory.join("Main.java");
    fs::write(
        &source,
        format!(
            "public final class Main {{ public static void main(String[] args) {{ {body} }} }}"
        ),
    )
    .expect("adapter source");
    let preparer = directory.join("com/voyantclair/callgraph/ArtifactPreparationMain.java");
    fs::create_dir_all(preparer.parent().expect("preparer parent")).expect("preparer directory");
    fs::write(
        &preparer,
        r#"package com.voyantclair.callgraph; public final class ArtifactPreparationMain { public static void main(String[] args) { System.out.print("{\"applicationClassDirs\":[\"/classes\"],\"testClassDirs\":[\"/test-classes\"],\"classpath\":[\"/dependency.jar\"],\"diagnostic\":\"\"}"); } }"#,
    )
    .expect("preparer source");
    assert!(
        Command::new("javac")
            .arg(&source)
            .arg(&preparer)
            .status()
            .expect("javac should start")
            .success()
    );
    let manifest = directory.join("MANIFEST.MF");
    fs::write(&manifest, "Main-Class: Main\n").expect("manifest");
    let jar = directory.join("adapter.jar");
    assert!(
        Command::new("jar")
            .args(["cfm"])
            .arg(&jar)
            .arg(&manifest)
            .arg("-C")
            .arg(directory)
            .arg(".")
            .status()
            .expect("jar should start")
            .success()
    );
    jar
}
