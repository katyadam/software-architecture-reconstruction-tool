#![cfg(unix)]
#[path = "support/go_package_runtime.rs"]
mod fixture_support;
use extractor_runtime::impact::go::{analyze_go_impact, analyze_go_package_impact};
use fixture_support::*;
use models::{go_impact::*, go_package_impact::*};
use std::{fs, time::Duration};
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
