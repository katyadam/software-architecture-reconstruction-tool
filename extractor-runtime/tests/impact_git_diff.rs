use extractor_runtime::impact::analyze_changes;
use models::ChangedElementKind;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

/// Removes the temporary Git fixture after its test completes.
struct TemporaryRepository(PathBuf);

impl Drop for TemporaryRepository {
    /// Deletes the test-only repository directory.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn maps_java_and_non_java_git_changes_to_conservative_elements() {
    let repository = create_repository();
    write_initial_revision(&repository.0);
    git(&repository.0, &["add", "."]);
    git(&repository.0, &["commit", "-m", "baseline"]);

    write_candidate_revision(&repository.0);
    git(&repository.0, &["add", "-A"]);
    git(&repository.0, &["commit", "-m", "candidate"]);

    let changes = analyze_changes(&repository.0, "HEAD~1").expect("Git changes are analyzed");

    assert!(changes.iter().any(|change| {
        change.source_path.ends_with("Service.java")
            && change.kind == ChangedElementKind::Callable
            && change.callable_signature.is_some()
    }));
    assert!(changes.iter().any(|change| {
        change.source_path.ends_with("Service.java") && change.kind == ChangedElementKind::Class
    }));
    for path in ["Removed.java", "Renamed.java", "application.yml", "pom.xml"] {
        assert!(changes.iter().any(|change| {
            change.source_path.ends_with(path) && change.kind == ChangedElementKind::Module
        }));
    }
}

/// Creates an isolated Git repository with a unique filesystem location.
fn create_repository() -> TemporaryRepository {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("voyantclair-impact-{suffix}"));
    fs::create_dir_all(&root).expect("repository directory is created");
    git(&root, &["init"]);
    git(&root, &["config", "user.email", "impact@example.test"]);
    git(&root, &["config", "user.name", "Impact Test"]);
    TemporaryRepository(root)
}

/// Writes the baseline Maven module and its initial source files.
fn write_initial_revision(root: &Path) {
    write(
        root,
        "pom.xml",
        "<project><modelVersion>4.0.0</modelVersion></project>",
    );
    write(
        root,
        "src/main/java/example/Service.java",
        "package example;\nclass Service {\n  int field;\n  int changed() { return 1; }\n}\n",
    );
    write(
        root,
        "src/main/java/example/Removed.java",
        "package example; class Removed {}",
    );
    write(
        root,
        "src/main/java/example/OldName.java",
        "package example; class OldName {}",
    );
}

/// Applies candidate changes covering callable, class, deletion, rename, resource, and POM cases.
fn write_candidate_revision(root: &Path) {
    write(
        root,
        "src/main/java/example/Service.java",
        "package example;\nclass Service {\n  int addedField;\n  int changed() { return 2; }\n}\n",
    );
    fs::remove_file(root.join("src/main/java/example/Removed.java")).expect("source is removed");
    fs::rename(
        root.join("src/main/java/example/OldName.java"),
        root.join("src/main/java/example/Renamed.java"),
    )
    .expect("source is renamed");
    write(
        root,
        "src/main/resources/application.yml",
        "feature: changed",
    );
    write(
        root,
        "pom.xml",
        "<project><modelVersion>4.0.0</modelVersion><version>2</version></project>",
    );
}

/// Writes one UTF-8 fixture file and creates its parent directories.
fn write(root: &Path, relative_path: &str, content: &str) {
    let path = root.join(relative_path);
    fs::create_dir_all(path.parent().expect("fixture file has a parent"))
        .expect("fixture parent directory is created");
    fs::write(path, content).expect("fixture file is written");
}

/// Runs one Git command in the temporary repository and requires success.
fn git(root: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .status()
        .expect("Git is available for fixture setup");
    assert!(status.success(), "git {arguments:?} failed");
}
