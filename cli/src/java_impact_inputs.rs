//! Source-root-aware helpers shared by configured Java impact orchestration.
use models::{Callable, ChangedElement, ChangedElementKind, Namespace};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub fn callgraph_targets_in_roots(
    source_roots: &[PathBuf],
    changes: &[ChangedElement],
    callables: &[Callable],
) -> Vec<String> {
    let mut targets = BTreeSet::new();
    for change in changes {
        for callable in callables {
            let path = Path::new(&callable.file_path);
            if !path.ends_with(Path::new(&change.source_path))
                || !source_roots.iter().any(|r| path.starts_with(r))
            {
                continue;
            }
            let Namespace::Class(class) = &callable.namespace else {
                continue;
            };
            let name = callable
                .name
                .split_once('(')
                .and_then(|(p, _)| p.split_whitespace().last());
            let matches = match change.kind {
                ChangedElementKind::Callable => {
                    if let Some(sig) = change.callable_signature.as_deref() {
                        // sig: "ClassName.method(types...)" for methods,
                        //      "ClassName(types...)"        for constructors.
                        // Split on first '(' to isolate the name part before params.
                        let name_part = sig.split_once('(').map_or(sig, |(n, _)| n);
                        if let Some((owner, member)) = name_part.rsplit_once('.') {
                            owner == class && name == Some(member)
                        } else {
                            // No dot: constructor — name_part is the class name.
                            name_part == class && callable.is_constructor
                        }
                    } else {
                        false
                    }
                }
                ChangedElementKind::Class => change.callable_signature.as_deref() == Some(class),
                ChangedElementKind::Module => false,
            };
            if !matches {
                continue;
            }
            let Some(owner) =
                extractor_runtime::java_analysis::declaring_type_for_callable(path, class)
            else {
                continue;
            };
            let member = if callable.is_constructor {
                Some("<init>")
            } else {
                name
            };
            if let Some(member) = member {
                targets.insert(format!("{owner}#{member}/{}", callable.parameters.len()));
            }
        }
    }
    targets.into_iter().collect()
}

pub fn load_configured_impact_inputs(
    project_root: &Path,
    config_file: Option<&Path>,
    config_base_dir: Option<&Path>,
    candidate_revision: &str,
) -> anyhow::Result<Option<extractor_runtime::java_analysis::ConfiguredJavaInputs>> {
    use anyhow::Context;
    let Some(file) = config_file else {
        anyhow::ensure!(
            config_base_dir.is_none(),
            "--config-base-dir requires --config-file"
        );
        return Ok(None);
    };
    let config: models::ConfigurationData = serde_json::from_slice(
        &std::fs::read(file)
            .with_context(|| format!("cannot read configuration {}", file.display()))?,
    )?;
    Ok(Some(
        extractor_runtime::java_analysis::load_java_analysis_inputs(
            project_root,
            config_base_dir.unwrap_or(project_root),
            &config,
            candidate_revision,
        )?,
    ))
}

pub fn module_source_layout(
    module: &Path,
    inputs: Option<&extractor_runtime::java_analysis::ConfiguredJavaInputs>,
) -> extractor_runtime::call_graph::JavaSourceLayout {
    if let Some(m) = inputs.and_then(|i| i.modules.get(module)) {
        return extractor_runtime::call_graph::JavaSourceLayout {
            production_roots: m.source_roots.clone(),
            test_roots: m.test_source_roots.clone(),
        };
    }
    extractor_runtime::call_graph::JavaSourceLayout {
        production_roots: vec![module.join("src/main/java")],
        test_roots: vec![module.join("src/test/java")],
    }
}

pub fn module_test_inputs(
    module: &Path,
    inputs: Option<&extractor_runtime::java_analysis::ConfiguredJavaInputs>,
) -> anyhow::Result<(Vec<models::JavaTestCase>, bool)> {
    if let Some(m) = inputs.and_then(|i| i.modules.get(module)) {
        return Ok((m.tests.clone(), m.test_inventory_complete));
    }
    Ok((
        extractor_runtime::impact::discover_java_tests(module)?,
        true,
    ))
}

pub fn apply_configured_execution_scope(
    result: &mut models::TestImpactResult,
    inputs: &extractor_runtime::java_analysis::ConfiguredJavaInputs,
    consumer_scope_complete: bool,
) {
    result.analysis_contexts = inputs.contexts.clone();
    if result.changed_elements.is_empty() {
        return;
    }
    if inputs.requires_full_test_run() || !consumer_scope_complete || result.requires_full_test_run
    {
        result.requires_full_test_run = true;
        let reason = "configured Java inventory/dependency scope incomplete; run the whole checkout suite in the recorded profile context; known tests are not a complete execution list";
        result.diagnostics.push(reason.into());
        for test in result.unselected_tests.drain(..) {
            result.selected_tests.push(models::SelectedTest {
                test,
                reasons: vec![models::SelectionReason::ModuleFallback {
                    diagnostic: reason.into(),
                }],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configured_java_roots_extracts_declared_roots() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source/java/fixture");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("Service.java"),"package fixture; class Service { Service() {} void run() {} class Inner { void run() {} } }").unwrap();
        let callables = crate::get_java_callables_in_roots_with_progress(
            &[dir.path().join("source/java")],
            false,
        )
        .unwrap();
        assert_eq!(callables.len(), 3);
        assert!(callables.iter().any(|c| c.is_constructor));
    }

    #[test]
    fn configured_java_roots_targets_unique_package_identity() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("source/java");
        for package in ["a", "b"] {
            std::fs::create_dir_all(root.join(package)).unwrap();
            std::fs::write(root.join(package).join("Service.java"),format!("package {package}; class Service {{ Service() {{}} void run() {{}} class Inner {{ void run() {{}} }} }}")).unwrap();
        }
        let callables =
            crate::get_java_callables_in_roots_with_progress(&[root.clone()], false).unwrap();
        let change = ChangedElement {
            module_root: dir.path().to_string_lossy().into(),
            source_path: "source/java/a/Service.java".into(),
            kind: ChangedElementKind::Class,
            callable_signature: Some("Service".into()),
        };
        assert_eq!(
            callgraph_targets_in_roots(&[root.clone()], &[change.clone()], &callables),
            ["La/Service#<init>/0", "La/Service#run/0"]
        );
        let change = ChangedElement {
            kind: ChangedElementKind::Callable,
            callable_signature: Some("Inner.run()".into()),
            ..change
        };
        assert_eq!(
            callgraph_targets_in_roots(&[root], &[change], &callables),
            ["La/Service$Inner#run/0"]
        );
    }

    #[test]
    fn configured_java_roots_matches_method_with_param_types() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("source/java");
        std::fs::create_dir_all(root.join("pkg")).unwrap();
        std::fs::write(
            root.join("pkg/Svc.java"),
            "package pkg; class Svc { int process(String name, int count) { return 0; } }",
        )
        .unwrap();
        let callables =
            crate::get_java_callables_in_roots_with_progress(&[root.clone()], false).unwrap();
        let change = ChangedElement {
            module_root: dir.path().to_string_lossy().into(),
            source_path: "source/java/pkg/Svc.java".into(),
            kind: ChangedElementKind::Callable,
            // Signature format now includes parameter types.
            callable_signature: Some("Svc.process(String, int)".into()),
        };
        assert_eq!(
            callgraph_targets_in_roots(&[root], &[change], &callables),
            ["Lpkg/Svc#process/2"]
        );
    }

    #[test]
    fn configured_java_roots_matches_constructor_with_param_types() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("source/java");
        std::fs::create_dir_all(root.join("pkg")).unwrap();
        std::fs::write(
            root.join("pkg/Order.java"),
            "package pkg; class Order { Order(String id) {} }",
        )
        .unwrap();
        let callables =
            crate::get_java_callables_in_roots_with_progress(&[root.clone()], false).unwrap();
        let change = ChangedElement {
            module_root: dir.path().to_string_lossy().into(),
            source_path: "source/java/pkg/Order.java".into(),
            kind: ChangedElementKind::Callable,
            // Constructor signature has no class prefix.
            callable_signature: Some("Order(String)".into()),
        };
        assert_eq!(
            callgraph_targets_in_roots(&[root], &[change], &callables),
            ["Lpkg/Order#<init>/1"]
        );
    }
}
