use extractor_runtime::impact::go::{map_go_changes, read_go_diff};
use models::{
    call_graph::{Language, MethodRef},
    go_impact::*,
};
use std::{fs, path::Path, process::Command};
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@example.test",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn repo(source: &str) -> (tempfile::TempDir, String) {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-q"]);
    fs::write(
        d.path().join("go.mod"),
        "module example.test/c\n\ngo 1.26.0\n",
    )
    .unwrap();
    fs::write(d.path().join("s.go"), source).unwrap();
    git(d.path(), &["add", "."]);
    git(d.path(), &["commit", "-qm", "baseline"]);
    let sha = git(d.path(), &["rev-parse", "HEAD"]);
    (d, sha)
}
fn commit(root: &Path) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "candidate"]);
}
fn metadata(
    root: &Path,
    old_end: u32,
    new_end: u32,
    member: &str,
) -> (GoInventoryOutcome, GoDescriptionOutcome) {
    let syn = |end| GoSourceDeclaration {
        member_name: member.into(),
        header: format!("{member}:func() int"),
        start_line: 2,
        end_line: end,
        body_start_line: 2,
        body_end_line: end,
    };
    let method = MethodRef {
        language: Language::Go,
        declaring_type: "example.test/c".into(),
        member_name: member.into(),
        descriptor: "func() int".into(),
        source_path: Some(root.join("s.go").to_string_lossy().into()),
        source_line: Some(2),
    };
    let inventory = GoInventoryOutcome {
        schema_version: 1,
        provider_id: "go-native".into(),
        status: GoStageStatus::Ok,
        source_root: root.to_string_lossy().into(),
        source_files: vec![root.join("s.go").to_string_lossy().into()],
        tests: vec![],
        declarations: vec![GoNativeDeclaration {
            method,
            syntax: syn(new_end),
        }],
        test_inventory_complete: true,
        scope: None,
        diagnostics: vec![],
    };
    let descriptions = GoDescriptionOutcome {
        schema_version: 1,
        provider_id: "go-native".into(),
        status: GoStageStatus::Ok,
        files: vec![
            GoFileDescription {
                source_path: "baseline:s.go".into(),
                package_name: Some("c".into()),
                declarations: vec![syn(old_end)],
                diagnostics: vec![],
            },
            GoFileDescription {
                source_path: "candidate:s.go".into(),
                package_name: Some("c".into()),
                declarations: vec![syn(new_end)],
                diagnostics: vec![],
            },
        ],
        diagnostics: vec![],
    };
    (inventory, descriptions)
}
#[test]
fn body_edit_maps_to_native_target() {
    let (d, b) = repo("package c\nfunc F() int { return 1 }\n");
    fs::write(
        d.path().join("s.go"),
        "package c\nfunc F() int { return 2 }\n",
    )
    .unwrap();
    commit(d.path());
    let diff = read_go_diff(d.path(), &b).unwrap();
    let (i, m) = metadata(&diff.module_root, 2, 2, "F");
    let out = map_go_changes(&diff, &i, &m);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].target.as_deref(), Some("example.test/c#F"));
}
#[test]
fn deletion_only_and_one_line_body_edits_are_not_lost() {
    let (d, b) = repo("package c\nfunc F() int {\n x:=1\n x++\n return x\n}\n");
    fs::write(
        d.path().join("s.go"),
        "package c\nfunc F() int {\n x:=1\n return x\n}\n",
    )
    .unwrap();
    commit(d.path());
    let diff = read_go_diff(d.path(), &b).unwrap();
    assert!(diff.files[0].hunks.iter().any(|h| h.new_count == 0));
    let (i, m) = metadata(&diff.module_root, 6, 5, "F");
    assert_eq!(
        map_go_changes(&diff, &i, &m)[0].kind,
        GoChangeKind::Callable
    );
}
#[test]
fn receiver_and_closure_edits_keep_owner() {
    let (d, b) = repo(
        "package c\nfunc (v *T) F() int { f:=func() int {return 1};return f() }\ntype T struct{}\n",
    );
    fs::write(
        d.path().join("s.go"),
        "package c\nfunc (v *T) F() int { f:=func() int {return 2};return f() }\ntype T struct{}\n",
    )
    .unwrap();
    commit(d.path());
    let diff = read_go_diff(d.path(), &b).unwrap();
    let (i, m) = metadata(&diff.module_root, 2, 2, "(*T).F");
    assert_eq!(
        map_go_changes(&diff, &i, &m)[0].target.as_deref(),
        Some("example.test/c#(*T).F")
    );
}
#[test]
fn multiple_hunks_deduplicate_target() {
    let (d, b) = repo("package c\nfunc F() int {\n a:=1\n b:=2\n c:=3\n return a+b+c\n}\n");
    fs::write(
        d.path().join("s.go"),
        "package c\nfunc F() int {\n a:=2\n b:=2\n c:=4\n return a+b+c\n}\n",
    )
    .unwrap();
    commit(d.path());
    let diff = read_go_diff(d.path(), &b).unwrap();
    let (i, m) = metadata(&diff.module_root, 7, 7, "F");
    assert_eq!(map_go_changes(&diff, &i, &m).len(), 1);
}
#[test]
fn declarations_globals_signatures_and_configuration_broaden() {
    let (d, b) = repo("package c\nfunc F() int {return 1}\n");
    fs::write(
        d.path().join("s.go"),
        "package c\nfunc F(x int) int {return x}\n",
    )
    .unwrap();
    fs::write(d.path().join("config.yml"), "x: 2").unwrap();
    commit(d.path());
    let diff = read_go_diff(d.path(), &b).unwrap();
    let (i, mut m) = metadata(&diff.module_root, 2, 2, "F");
    m.files[1].declarations[0].header = "F:func(x int) int".into();
    let out = map_go_changes(&diff, &i, &m);
    assert_eq!(out.len(), 2);
    assert!(out.iter().all(|c| c.kind == GoChangeKind::Module));
}
#[test]
fn unusual_paths_are_not_patch_headers_or_options() {
    let (d, b) = repo("package c");
    for p in [
        "a b.go",
        "unicodé.go",
        "tab\t.go",
        "newline\n.go",
        "-odd.go",
    ] {
        fs::write(d.path().join(p), "package c").unwrap();
    }
    commit(d.path());
    let diff = read_go_diff(d.path(), &b).unwrap();
    assert_eq!(diff.files.len(), 5);
    assert!(
        diff.files
            .iter()
            .any(|f| f.new_path.as_deref() == Some("newline\n.go"))
    );
}
#[test]
fn renames_keep_both_paths() {
    let (d, b) = repo("package c\nfunc F() int {return 1}\n");
    fs::rename(d.path().join("s.go"), d.path().join("renamed.go")).unwrap();
    commit(d.path());
    let diff = read_go_diff(d.path(), &b).unwrap();
    assert_eq!(diff.files[0].old_path.as_deref(), Some("s.go"));
    assert_eq!(diff.files[0].new_path.as_deref(), Some("renamed.go"));
    let (i, m) = metadata(&diff.module_root, 2, 2, "F");
    assert_eq!(map_go_changes(&diff, &i, &m)[0].kind, GoChangeKind::Module);
}
#[test]
fn dirty_invalid_revision_and_unsupported_module_scope_are_rejected() {
    let (d, b) = repo("package c");
    assert!(read_go_diff(d.path(), "--help").is_err());
    fs::write(d.path().join("s.go"), "package changed").unwrap();
    assert!(read_go_diff(d.path(), &b).is_err());
    commit(d.path());
    fs::create_dir(d.path().join("nested")).unwrap();
    fs::write(d.path().join("nested/go.mod"), "module nested").unwrap();
    commit(d.path());
    assert!(read_go_diff(d.path(), &b).is_err());
}
#[test]
fn linked_worktree_is_accepted() {
    let (d, b) = repo("package c");
    let out = tempfile::tempdir().unwrap();
    let path = out.path().join("linked");
    git(
        d.path(),
        &[
            "worktree",
            "add",
            "--detach",
            path.to_str().unwrap(),
            "HEAD",
        ],
    );
    assert!(read_go_diff(&path, &b).unwrap().files.is_empty());
}
