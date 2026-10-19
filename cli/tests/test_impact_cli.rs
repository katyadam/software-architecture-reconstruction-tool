use std::{fs, path::Path, process::Command, sync::Mutex};
use tempfile::TempDir;

/// Keeps external Maven and WALA processes from competing for the test runner's resources.
static TIA_PROCESS_LOCK: Mutex<()> = Mutex::new(());

/// Runs the standalone binary against a two-version Maven fixture and reads its JSON output.
#[test]
fn selects_the_junit_test_that_reaches_a_changed_method() {
    let fixture = create_fixture();
    let baseline = git(&fixture, &["rev-parse", "HEAD"]);
    fs::write(
        fixture.path().join("src/main/java/fixture/Service.java"),
        "package fixture; public class Service { public static void changed() { int value = 2; } public static void unchanged() {} }",
    )
    .expect("candidate source writes");
    git(&fixture, &["add", "."]);
    git(&fixture, &["commit", "-m", "candidate"]);

    let result = run_tia(&fixture, &baseline);
    assert_eq!(result.selected_tests.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(result.selected_tests[0].test.method_name, "coversChanged");
    assert!(matches!(
        result.selected_tests[0].reasons.as_slice(),
        [models::SelectionReason::ChangedCallable { .. }]
    ));
    assert_timing_diagnostics(
        &result,
        [
            "git diff",
            "source extraction",
            "test discovery",
            "WALA analysis",
            "edge resolution",
            "test selection",
        ],
    );
}

#[test]
fn quiet_mode_writes_a_result_without_extraction_progress() {
    let fixture = create_fixture();
    let baseline = git(&fixture, &["rev-parse", "HEAD"]);
    fs::write(
        fixture.path().join("src/main/java/fixture/Service.java"),
        "package fixture; public class Service { public static void changed() { int value = 2; } public static void unchanged() {} }",
    )
    .expect("candidate source writes");
    git(&fixture, &["add", "."]);
    git(&fixture, &["commit", "-m", "candidate"]);

    let (result, output) = run_tia_with_extra_args(&fixture, &baseline, &wala_jar(), &["--quiet"]);

    assert!(
        !result.selected_tests.is_empty(),
        "{:?}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.starts_with("timing: source extraction=")),
        "quiet mode must still run source extraction: {:?}",
        result.diagnostics
    );
    assert!(
        output.stdout.is_empty(),
        "quiet mode wrote progress to stdout"
    );
}

#[test]
fn selects_every_module_test_for_a_non_java_configuration_change() {
    let fixture = create_fixture();
    let baseline = git(&fixture, &["rev-parse", "HEAD"]);
    write_source(
        fixture.path(),
        "src/main/resources/application.properties",
        "service.mode=changed",
    );
    git(&fixture, &["add", "."]);
    git(&fixture, &["commit", "-m", "configuration candidate"]);

    assert_module_fallback(&run_tia(&fixture, &baseline));
}

#[test]
fn selects_module_tests_without_a_wala_adapter_for_a_configuration_change() {
    let fixture = create_fixture();
    let baseline = git(&fixture, &["rev-parse", "HEAD"]);
    write_source(
        fixture.path(),
        "src/main/resources/application.properties",
        "service.mode=changed",
    );
    git(&fixture, &["add", "."]);
    git(&fixture, &["commit", "-m", "configuration candidate"]);

    let result = run_tia_with_jar(
        &fixture,
        &baseline,
        &fixture.path().join("missing-wala.jar"),
    );

    assert_module_fallback(&result);
    assert_timing_diagnostics(
        &result,
        [
            "git diff",
            "module fallback test discovery",
            "module fallback selection",
        ],
    );
}

#[test]
fn selects_every_module_test_for_a_deleted_java_source_file() {
    let fixture = create_fixture();
    let baseline = git(&fixture, &["rev-parse", "HEAD"]);
    fs::remove_file(fixture.path().join("src/main/java/fixture/Service.java"))
        .expect("candidate source deletes");
    git(&fixture, &["add", "-u"]);
    git(&fixture, &["commit", "-m", "deletion candidate"]);

    assert_module_fallback(&run_tia(&fixture, &baseline));
}

#[test]
fn selects_descendant_module_tests_for_a_reactor_root_configuration_change() {
    let fixture = create_reactor_fixture();
    let baseline = git(&fixture, &["rev-parse", "HEAD"]);
    fs::write(
        fixture.path().join("pom.xml"),
        "<project xmlns=\"http://maven.apache.org/POM/4.0.0\"><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>reactor</artifactId><version>1</version><packaging>pom</packaging><modules><module>orders</module><module>billing</module></modules><properties><maven.compiler.release>8</maven.compiler.release><fixture.mode>baseline</fixture.mode></properties></project>",
    )
    .expect("candidate reactor POM writes");
    git(&fixture, &["add", "."]);
    git(
        &fixture,
        &["commit", "-m", "reactor configuration candidate"],
    );

    assert_module_fallback(&run_tia(&fixture, &baseline));
}

#[test]
fn selects_each_descendant_test_once_when_root_and_child_modules_change() {
    let fixture = create_reactor_fixture();
    let baseline = git(&fixture, &["rev-parse", "HEAD"]);
    fs::write(
        fixture.path().join("pom.xml"),
        "<project xmlns=\"http://maven.apache.org/POM/4.0.0\"><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>reactor</artifactId><version>1</version><packaging>pom</packaging><modules><module>orders</module><module>billing</module></modules><properties><maven.compiler.release>8</maven.compiler.release><fixture.mode>candidate</fixture.mode></properties></project>",
    )
    .expect("candidate reactor POM writes");
    write_source(
        fixture.path(),
        "orders/src/main/java/fixture/orders/Service.java",
        "package fixture.orders; public class Service { public static void run() { int value = 2; } }",
    );
    git(&fixture, &["add", "."]);
    git(
        &fixture,
        &["commit", "-m", "root and child module candidate"],
    );

    assert_module_fallback(&run_tia(&fixture, &baseline));
}

/// Executes the binary for one candidate fixture and deserializes its persisted selection result.
fn run_tia(fixture: &TempDir, baseline: &str) -> models::TestImpactResult {
    run_tia_with_jar(fixture, baseline, &wala_jar())
}

/// Runs TIA with an explicit adapter path so fallback paths can prove they do not require WALA.
fn run_tia_with_jar(
    fixture: &TempDir,
    baseline: &str,
    wala_adapter_jar: &Path,
) -> models::TestImpactResult {
    run_tia_with_extra_args(fixture, baseline, wala_adapter_jar, &[]).0
}

/// Executes TIA with optional command-line switches and returns its output for CLI behavior checks.
fn run_tia_with_extra_args(
    fixture: &TempDir,
    baseline: &str,
    wala_adapter_jar: &Path,
    extra_args: &[&str],
) -> (models::TestImpactResult, std::process::Output) {
    let _process_guard = TIA_PROCESS_LOCK
        .lock()
        .expect("TIA process lock is available");
    let output = fixture.path().join("impact.json");
    let mut command = Command::new(env!("CARGO_BIN_EXE_test-impact"));
    command
        .args([
            "--project-dir",
            fixture.path().to_str().expect("fixture path is UTF-8"),
            "--baseline-revision",
            baseline,
            "--wala-adapter-jar",
            wala_adapter_jar.to_str().expect("adapter path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
        ])
        .args(extra_args);
    let output_result = command.output().expect("TIA binary starts");
    assert!(
        output_result.status.success(),
        "{}",
        String::from_utf8_lossy(&output_result.stderr)
    );
    (
        serde_json::from_str(&fs::read_to_string(output).expect("TIA result is written"))
            .expect("TIA result is JSON"),
        output_result,
    )
}

/// Verifies the controlled fallback contract keeps every discovered module test.
fn assert_module_fallback(result: &models::TestImpactResult) {
    assert_eq!(result.selected_tests.len(), 2, "{:?}", result.diagnostics);
    assert!(result.unselected_tests.is_empty());
    assert!(result.selected_tests.iter().all(|selected| {
        selected
            .reasons
            .iter()
            .any(|reason| matches!(reason, models::SelectionReason::ModuleFallback { .. }))
    }));
}

/// Verifies timing diagnostics have stable stage labels while allowing variable durations.
fn assert_timing_diagnostics<const N: usize>(
    result: &models::TestImpactResult,
    expected_stages: [&str; N],
) {
    for stage in expected_stages {
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.starts_with(&format!("timing: {stage}="))),
            "missing timing diagnostic for {stage}: {:?}",
            result.diagnostics
        );
    }
}

/// Creates and commits a compile-only Maven project with one recognized JUnit-style test.
fn create_fixture() -> TempDir {
    let fixture = tempfile::tempdir().expect("temporary fixture directory");
    fs::write(
        fixture.path().join("pom.xml"),
        "<project xmlns=\"http://maven.apache.org/POM/4.0.0\"><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>service</artifactId><version>1</version><properties><maven.compiler.release>8</maven.compiler.release></properties></project>",
    )
    .expect("POM writes");
    write_source(
        fixture.path(),
        "src/main/java/fixture/Service.java",
        "package fixture; public class Service { public static void changed() { int value = 1; } public static void unchanged() {} }",
    );
    write_source(
        fixture.path(),
        "src/test/java/org/junit/jupiter/api/Test.java",
        "package org.junit.jupiter.api; public @interface Test {}",
    );
    write_source(
        fixture.path(),
        "src/test/java/fixture/ServiceTest.java",
        "package fixture; public class ServiceTest { @org.junit.jupiter.api.Test public void coversChanged() { Service.changed(); } @org.junit.jupiter.api.Test public void coversUnchanged() { Service.unchanged(); } }",
    );
    git(&fixture, &["init"]);
    git(&fixture, &["config", "user.email", "tia@example.test"]);
    git(&fixture, &["config", "user.name", "TIA fixture"]);
    git(&fixture, &["add", "."]);
    git(&fixture, &["commit", "-m", "baseline"]);
    fixture
}

/// Creates a two-module Maven reactor whose tests belong to child module roots.
fn create_reactor_fixture() -> TempDir {
    let fixture = tempfile::tempdir().expect("temporary reactor directory");
    fs::write(
        fixture.path().join("pom.xml"),
        "<project xmlns=\"http://maven.apache.org/POM/4.0.0\"><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>reactor</artifactId><version>1</version><packaging>pom</packaging><modules><module>orders</module><module>billing</module></modules><properties><maven.compiler.release>8</maven.compiler.release></properties></project>",
    )
    .expect("reactor POM writes");
    for module in ["orders", "billing"] {
        write_source(
            fixture.path(),
            &format!("{module}/src/main/java/fixture/{module}/Service.java"),
            &format!(
                "package fixture.{module}; public class Service {{ public static void run() {{}} }}"
            ),
        );
        write_source(
            fixture.path(),
            &format!("{module}/src/test/java/fixture/{module}/ServiceTest.java"),
            &format!(
                "package fixture.{module}; public class ServiceTest {{ @org.junit.jupiter.api.Test public void coversService() {{ Service.run(); }} }}"
            ),
        );
        fs::write(
            fixture.path().join(format!("{module}/pom.xml")),
            format!(
                "<project xmlns=\"http://maven.apache.org/POM/4.0.0\"><modelVersion>4.0.0</modelVersion><parent><groupId>fixture</groupId><artifactId>reactor</artifactId><version>1</version></parent><artifactId>{module}</artifactId></project>"
            ),
        )
        .expect("module POM writes");
        write_source(
            fixture.path(),
            &format!("{module}/src/test/java/org/junit/jupiter/api/Test.java"),
            "package org.junit.jupiter.api; public @interface Test {}",
        );
    }
    git(&fixture, &["init"]);
    git(&fixture, &["config", "user.email", "tia@example.test"]);
    git(&fixture, &["config", "user.name", "TIA fixture"]);
    git(&fixture, &["add", "."]);
    git(&fixture, &["commit", "-m", "reactor baseline"]);
    fixture
}

/// Writes fixture source while creating all missing source directories.
fn write_source(root: &Path, relative_path: &str, content: &str) {
    let path = root.join(relative_path);
    fs::create_dir_all(path.parent().expect("source parent exists"))
        .expect("source directory writes");
    fs::write(path, content).expect("source writes");
}

/// Runs a Git command in the fixture and returns its trimmed standard output.
fn git(fixture: &TempDir, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(fixture.path())
        .output()
        .expect("Git starts");
    assert!(
        output.status.success(),
        "git {:?} failed:\nstdout: {}\nstderr: {}",
        arguments,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Locates the adapter JAR built by the Maven-focused verification command.
fn wala_jar() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("wala-callgraph/target/wala-callgraph-all.jar")
}
