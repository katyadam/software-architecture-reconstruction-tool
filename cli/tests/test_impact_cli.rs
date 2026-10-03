use std::{fs, path::Path, process::Command};
use tempfile::TempDir;

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

    let output = fixture.path().join("impact.json");
    let status = Command::new(env!("CARGO_BIN_EXE_test-impact"))
        .args([
            "--project-dir",
            fixture.path().to_str().expect("fixture path is UTF-8"),
            "--baseline-revision",
            &baseline,
            "--wala-adapter-jar",
            wala_jar().to_str().expect("adapter path is UTF-8"),
            "--output",
            output.to_str().expect("output path is UTF-8"),
        ])
        .status()
        .expect("TIA binary starts");

    assert!(status.success());
    let result: models::TestImpactResult =
        serde_json::from_str(&fs::read_to_string(output).expect("TIA result is written"))
            .expect("TIA result is JSON");
    assert_eq!(result.selected_tests.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(result.selected_tests[0].test.method_name, "coversChanged");
    assert!(matches!(
        result.selected_tests[0].reasons.as_slice(),
        [models::SelectionReason::ChangedCallable { .. }]
    ));
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
        "{}",
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
