#![cfg(unix)]
#[path = "go_packages.rs"]
mod support;
use extractor_runtime::impact::go::GoImpactOptions;
use models::{go_impact::*, go_package_impact::*};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command, time::Duration};
pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub options: GoImpactOptions,
    // Shared with the CLI integration target, which mutates this capability.
    #[allow(dead_code)]
    pub evidence: GoPackageInventoryOutcome,
}
pub fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
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
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
pub fn commit(f: &Fixture) {
    git(&f.options.project_dir, &["add", "-A"]);
    git(&f.options.project_dir, &["commit", "-qm", "candidate"]);
}
pub fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("module");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let (mut i, mut e) = support::fixture(&root);
    fs::write(root.join("go.mod"), "module example\n\ngo 1.26.0\n").unwrap();
    fs::write(
        root.join("a/a.go"),
        "package a\nfunc Value() int {\n return 1\n}\n",
    )
    .unwrap();
    fs::create_dir(root.join("b")).unwrap();
    fs::write(
        root.join("b/b_test.go"),
        "package b; import \"testing\"; func TestB(t *testing.T) {}",
    )
    .unwrap();
    let source = root.join("b/b_test.go").to_string_lossy().into_owned();
    i.source_files.push(source.clone());
    let mut test = i.tests[0].clone();
    test.method.declaring_type = "example/b".into();
    test.method.member_name = "TestB".into();
    test.method.source_path = Some(source.clone());
    test.execution_package = "example/b".into();
    i.tests.push(test);
    e.packages.push(serde_json::from_value(serde_json::json!({"id":"binary-b","import_path":"example/b","imports":[],"provenance":{"kind":"verified_module","directory":root.join("b"),"module_path":"example"},"source_files":[source],"test_files":[source],"for_test":"example/b"})).unwrap());
    e.execution_packages.push(GoExecutionPackage {
        import_path: "example/b".into(),
        relative_directory: "b".into(),
        roots: vec!["binary-b".into()],
    });
    let syntax = GoSourceDeclaration {
        member_name: "Value".into(),
        header: "Value:func() int".into(),
        start_line: 2,
        end_line: 4,
        body_start_line: 2,
        body_end_line: 4,
    };
    let mut method = i.tests[0].method.clone();
    method.member_name = "Value".into();
    method.descriptor = "func() int".into();
    method.source_path = Some(root.join("a/a.go").to_string_lossy().into_owned());
    method.source_line = Some(2);
    i.declarations.push(GoNativeDeclaration {
        method,
        syntax: syntax.clone(),
    });
    let descriptions = GoDescriptionOutcome {
        schema_version: 1,
        provider_id: "go-native".into(),
        status: GoStageStatus::Ok,
        files: ["baseline", "candidate"]
            .iter()
            .map(|label| GoFileDescription {
                source_path: format!("{label}:a/a.go"),
                package_name: Some("a".into()),
                declarations: vec![syntax.clone()],
                diagnostics: vec![],
            })
            .collect(),
        diagnostics: vec![],
    };
    fs::write(
        dir.path().join("inventory"),
        serde_json::to_vec(&i).unwrap(),
    )
    .unwrap();
    fs::write(
        dir.path().join("describe"),
        serde_json::to_vec(&descriptions).unwrap(),
    )
    .unwrap();
    fs::write(dir.path().join("packages"), serde_json::to_vec(&e).unwrap()).unwrap();
    let peer = dir.path().join("peer.sh");
    let base = dir.path().display();
    fs::write(&peer,format!("#!/bin/sh\nif [ \"$1\" = --mode ]; then\n echo \"$2\" >> '{base}/calls'\n case \"$2\" in\n inventory) cat '{base}/inventory';;\n describe) cat >/dev/null; cat '{base}/describe';;\n package-inventory) cat '{base}/packages';;\n esac\nelse\n echo graph >> '{base}/calls'; exit 1\nfi\n")).unwrap();
    fs::set_permissions(&peer, fs::Permissions::from_mode(0o755)).unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "baseline"]);
    let baseline = git(&root, &["rev-parse", "HEAD"]);
    Fixture {
        dir,
        options: GoImpactOptions {
            project_dir: root,
            baseline_revision: baseline,
            go_callgraph_binary: peer,
            build_tags: String::new(),
            analysis_timeout: Duration::from_secs(2),
        },
        evidence: e,
    }
}
pub fn mutate(f: &Fixture) {
    fs::write(
        f.options.project_dir.join("a/a.go"),
        "package a\nfunc Value() int {\n return 2\n}\n",
    )
    .unwrap();
    commit(f);
}
