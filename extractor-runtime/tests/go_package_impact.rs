#![cfg(unix)]
#[path = "support/go_packages.rs"]
mod support;
use extractor_runtime::impact::go::{
    GoImpactOptions, analyze_go_impact, analyze_go_package_impact,
};
use models::{go_impact::*, go_package_impact::*};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command, time::Duration};
pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub options: GoImpactOptions,
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
fn packages(f: &Fixture) -> GoPackageImpactReport {
    analyze_go_package_impact(&f.options, GoPackageTestScope::LinkedBinary).unwrap()
}

#[test]
fn package_policy_is_independent_of_graph_failure_and_preserves_default() {
    let f = fixture();
    mutate(&f);
    let p = packages(&f);
    assert_eq!(p.base.schema_version, 2);
    assert_eq!(p.base.selection_mode, GoSelectionMode::ConservativePackages);
    assert_eq!(p.base.analysis.status, GoStageStatus::Failed);
    assert_eq!(p.selected_execution_packages[0].argument, "./a");
    assert_eq!(p.unselected_execution_packages[0].argument, "./b");
    assert!(!p.base.run_all_tests);
    assert_eq!(p.base.selected_tests.len(), 1);
    assert!(p.execution_inventory_complete);
    assert_eq!(
        fs::read_to_string(f.dir.path().join("calls")).unwrap(),
        "inventory\ndescribe\npackage-inventory\ngraph\n"
    );
    fs::remove_file(f.dir.path().join("calls")).unwrap();
    let old = analyze_go_impact(&f.options).unwrap();
    assert_eq!(old.schema_version, 1);
    assert_eq!(old.selected_tests.len(), 2);
    assert!(old.run_all_tests);
    assert!(
        !fs::read_to_string(f.dir.path().join("calls"))
            .unwrap()
            .contains("package-inventory")
    );
}
#[test]
fn empty_diff_is_not_a_verified_empty_execution_inventory() {
    let f = fixture();
    let p = packages(&f);
    assert_eq!(p.base.selection_mode, GoSelectionMode::NoChanges);
    assert!(!p.base.run_all_tests);
    assert!(!p.execution_inventory_complete);
    assert_eq!(p.package_evidence.status, GoStageStatus::Skipped);
    assert!(p.effective_build_context.is_none());
    assert!(!f.dir.path().join("calls").exists());
}
#[test]
fn package_timeout_or_legacy_response_retains_ordinary_test_inventory() {
    for legacy in [false, true] {
        let mut f = fixture();
        mutate(&f);
        let script = fs::read_to_string(&f.options.go_callgraph_binary).unwrap();
        let reply = if legacy {
            "exit 2".to_string()
        } else {
            "sleep 10 & wait".to_string()
        };
        let script = script.replace(
            &format!("cat '{}/packages'", f.dir.path().display()),
            &reply,
        );
        fs::write(&f.options.go_callgraph_binary, script).unwrap();
        // Ordinary shell startup must finish even when Git fixture tests run
        // concurrently. The package worker still sleeps beyond this deadline.
        f.options.analysis_timeout = Duration::from_secs(5);
        let p = packages(&f);
        assert_eq!(
            p.base.discovery.status,
            GoStageStatus::Ok,
            "{:?}",
            p.base.diagnostics
        );
        assert_eq!(
            p.base.mapping.status,
            GoStageStatus::Ok,
            "{:?}",
            p.base.diagnostics
        );
        assert_eq!(
            p.package_evidence.status,
            GoStageStatus::Failed,
            "{:?}",
            p.base.diagnostics
        );
        assert_eq!(p.base.discovered_tests.len(), 2);
        assert!(p.base.test_inventory_complete);
        assert!(!p.execution_inventory_complete);
        assert!(p.discovered_execution_packages.is_empty());
        assert!(p.effective_build_context.is_none());
        assert!(p.base.run_all_tests);
    }
}
#[test]
fn broad_changes_skip_package_metadata_and_preserve_input_errors() {
    for case in ["config", "signature", "deletion", "init", "import"] {
        let f = fixture();
        match case {
            "config" => {
                fs::write(f.options.project_dir.join("config.yml"), "changed: true").unwrap()
            }
            "deletion" => fs::remove_file(f.options.project_dir.join("a/a.go")).unwrap(),
            "signature" => fs::write(
                f.options.project_dir.join("a/a.go"),
                "package a\nfunc Value(x int) int {return x}\n",
            )
            .unwrap(),
            "init" => fs::write(
                f.options.project_dir.join("a/a.go"),
                "package a\nfunc Value() int {return 1}\nfunc init() {}\n",
            )
            .unwrap(),
            _ => fs::write(
                f.options.project_dir.join("a/a.go"),
                "package a\nimport _ \"fmt\"\nfunc Value() int {return 1}\n",
            )
            .unwrap(),
        };
        commit(&f);
        let p = packages(&f);
        assert!(p.base.run_all_tests, "{case}");
        assert_eq!(p.package_evidence.status, GoStageStatus::Skipped, "{case}");
        assert!(
            !fs::read_to_string(f.dir.path().join("calls"))
                .unwrap()
                .contains("package-inventory"),
            "{case}"
        );
    }
    let f = fixture();
    mutate(&f);
    fs::write(f.options.project_dir.join("dirty.go"), "package example").unwrap();
    assert!(analyze_go_package_impact(&f.options, GoPackageTestScope::LinkedBinary).is_err());
}
#[test]
fn changed_sources_during_analysis_are_not_published() {
    let f = fixture();
    mutate(&f);
    let script = fs::read_to_string(&f.options.go_callgraph_binary)
        .unwrap()
        .replace(
            "echo graph",
            &format!(
                "echo dirty > '{}/a/a.go'; echo graph",
                f.options.project_dir.display()
            ),
        );
    fs::write(&f.options.go_callgraph_binary, script).unwrap();
    assert!(analyze_go_package_impact(&f.options, GoPackageTestScope::LinkedBinary).is_err());
}
