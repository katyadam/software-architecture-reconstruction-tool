use extractor_runtime::{
    impact::{
        ModuleCatalog, ModuleDependencyGraph, analyze_changes_with_catalog,
        discover_java_tests_in_roots,
    },
    java_analysis::{ConfiguredJavaInputs, JavaBytecodeInputs, ResolvedJavaModule},
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn module(root: PathBuf) -> ResolvedJavaModule {
    ResolvedJavaModule {
        source_roots: vec![root.join("source/java")],
        test_source_roots: vec![root.join("checks/java")],
        root,
        bytecode: JavaBytecodeInputs::default(),
        dependencies: BTreeSet::new(),
        dependency_inventory_complete: true,
        test_inventory_complete: true,
        tests: vec![],
    }
}
fn write(root: &Path, path: &str, code: &str) {
    let p = root.join(path);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, code).unwrap();
}
fn git(root: &Path, args: &[&str]) {
    let o = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn declared_roots_narrow_changed_methods_without_losing_unlisted_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.email", "fixture@example.invalid"]);
    git(&root, &["config", "user.name", "Fixture"]);
    write(
        &root,
        "app/source/java/Service.java",
        "class Service {\n int value() { return 1; }\n}\n",
    );
    write(&root, "shared/values.txt", "old");
    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "baseline"]);
    write(
        &root,
        "app/source/java/Service.java",
        "class Service {\n int value() { return 2; }\n}\n",
    );
    write(&root, "shared/values.txt", "new");
    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "candidate"]);
    let mut inputs = ConfiguredJavaInputs {
        module_inventory_complete: true,
        ..Default::default()
    };
    inputs
        .modules
        .insert(root.join("app"), module(root.join("app")));
    let catalog = ModuleCatalog::load(&root).with_configured_inputs(&inputs);
    let changes = analyze_changes_with_catalog(&root, "HEAD~1", &catalog).unwrap();
    assert!(
        changes
            .iter()
            .any(|c| c.kind == models::ChangedElementKind::Callable
                && c.module_root == root.join("app").to_string_lossy())
    );
    assert!(
        changes.iter().any(|c| c.source_path == "shared/values.txt"
            && c.kind == models::ChangedElementKind::Module)
    );
    assert!(!catalog.can_narrow_source("unknown/Thing.java"));
}

#[test]
fn supplied_dependencies_find_consumers_and_unknown_builds_block_independence() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let mut inputs = ConfiguredJavaInputs {
        module_inventory_complete: true,
        ..Default::default()
    };
    for name in ["shared", "app", "independent"] {
        fs::create_dir(root.join(name)).unwrap();
        inputs
            .modules
            .insert(root.join(name), module(root.join(name)));
    }
    inputs
        .modules
        .get_mut(&root.join("app"))
        .unwrap()
        .dependencies
        .insert(root.join("shared"));
    let graph = ModuleDependencyGraph::load_with_configured_inputs(&root, &inputs).unwrap();
    assert_eq!(
        graph.consumers_of(&root.join("shared")),
        BTreeSet::from([root.join("app")])
    );
    // An unknown build discovered by the filesystem scan blocks independence only when the
    // automatic dependency graph succeeded (i.e. a supported reactor file exists at root).
    // When the automatic graph fails (no reactor, unsupported syntax), the manifest's
    // module_inventory_complete=true claim is trusted as-is.
    write(&root, "unknown/pom.xml", "<project/>");
    // No root pom.xml/settings.gradle means automatic graph fails → check skipped → no error.
    assert!(
        ModuleDependencyGraph::load_with_configured_inputs(&root, &inputs).is_ok(),
        "without an automatic graph, manifest module_inventory_complete is trusted"
    );
    // With a root reactor that claims exactly the three known modules, the unknown/pom.xml
    // is detectable (automatic graph succeeds, but unknown is outside it) and must still block.
    let pom = |g: &str, a: &str| {
        format!(
            "<project><groupId>{g}</groupId><artifactId>{a}</artifactId><version>1</version></project>"
        )
    };
    write(
        &root,
        "pom.xml",
        "<project><groupId>x</groupId><artifactId>root</artifactId><version>1</version>\
         <modules><module>shared</module><module>app</module><module>independent</module></modules></project>",
    );
    write(&root, "shared/pom.xml", &pom("x", "shared"));
    write(&root, "app/pom.xml", &pom("x", "app"));
    write(&root, "independent/pom.xml", &pom("x", "independent"));
    assert!(
        ModuleDependencyGraph::load_with_configured_inputs(&root, &inputs).is_err(),
        "automatic graph knows 3 modules but finds unknown/pom.xml outside reactor — must block"
    );
}

#[test]
fn declared_test_roots_and_generated_sources_are_discovered() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(
        &root,
        "checks/java/example/ServiceTest.java",
        "package example; class ServiceTest { @Test void works() {} }",
    );
    write(
        &root,
        "generated/checks/example/GeneratedTest.java",
        "package example; class GeneratedTest { @Test void generated() {} }",
    );
    let tests = discover_java_tests_in_roots(
        &root,
        &[root.join("checks/java"), root.join("generated/checks")],
    )
    .unwrap();
    assert_eq!(tests.len(), 2);
    assert!(tests.iter().any(|t| t.method_name == "generated"));
}

#[test]
fn nested_build_module_remains_deepest_owner_under_a_configured_service() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(
        &root,
        "pom.xml",
        "<project><modules><module>app</module></modules></project>",
    );
    write(
        &root,
        "app/pom.xml",
        "<project><modules><module>child</module></modules></project>",
    );
    write(&root, "app/child/pom.xml", "<project/>");
    let mut inputs = ConfiguredJavaInputs::default();
    inputs
        .services
        .push(extractor_runtime::java_analysis::ConfiguredJavaService {
            name: "app".into(),
            root: root.join("app"),
            manifest_path: None,
        });
    let catalog = ModuleCatalog::load(&root).with_configured_inputs(&inputs);
    assert_eq!(
        catalog.owner_for("app/child/src/main/java/Thing.java"),
        root.join("app/child")
    );
}

#[test]
fn complete_reactor_does_not_hide_unlisted_build_consumers() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(
        &root,
        "pom.xml",
        "<project><groupId>x</groupId><artifactId>root</artifactId><version>1</version></project>",
    );
    write(&root, "outside/pom.xml", "<project/>");
    let mut inputs = ConfiguredJavaInputs {
        module_inventory_complete: true,
        ..Default::default()
    };
    inputs.modules.insert(root.clone(), module(root.clone()));
    assert!(ModuleDependencyGraph::load_with_configured_inputs(&root, &inputs).is_err());
}

#[test]
fn gradle_submodule_excluded_from_manifest_does_not_block_precise_analysis() {
    // A Gradle submodule that is listed in settings.gradle but intentionally excluded
    // from inputs.modules (e.g. an integration-test-only module) should not cause the
    // dependency graph to error. The manifest is authoritative when
    // module_inventory_complete=true and the automatic loader already claimed the module.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(
        &root,
        "settings.gradle",
        "rootProject.name = 'app'\ninclude ':svc'\ninclude ':excluded'",
    );
    write(&root, "build.gradle", "");
    write(&root, "svc/build.gradle", "");
    write(&root, "excluded/build.gradle", "");
    let mut inputs = ConfiguredJavaInputs {
        module_inventory_complete: true,
        ..Default::default()
    };
    // Only 'svc' in manifest; 'excluded' is intentionally absent.
    inputs
        .modules
        .insert(root.join("svc"), module(root.join("svc")));
    let graph = ModuleDependencyGraph::load_with_configured_inputs(&root, &inputs);
    assert!(
        graph.is_ok(),
        "excluded known submodule should not cause fallback: {:?}",
        graph.err()
    );
}
