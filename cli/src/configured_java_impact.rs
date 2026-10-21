//! Java TIA orchestration over explicit and automatic module inputs.
use crate::java_impact_inputs::{
    apply_configured_execution_scope, callgraph_targets_in_roots, module_source_layout,
};
use extractor_runtime::{
    call_graph::{
        JavaSourceLayout, java_adapter::JavaAdapterProvider, resolve_reachability_with_java_layout,
        wala::WalaJavaProvider,
    },
    impact::{
        ModuleDependencyGraph, discover_java_tests_in_checkout, discover_java_tests_in_roots,
        select_tests_with_reachability,
    },
    java_analysis::{ConfiguredJavaInputs, JavaBytecodeInputs},
};
use models::{
    ChangedElement, ChangedElementKind, JavaTestCase, SelectedTest, SelectionReason,
    TestImpactResult,
    call_graph::{CallGraphOutcome, CallGraphStatus},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Instant,
};

pub trait ConfiguredJavaProvider {
    fn label(&self) -> &str;
    fn prepare_module(&self, module: &Path) -> Result<JavaBytecodeInputs, String>;
    fn analyze_prepared(
        &self,
        root: &Path,
        tests: &[JavaTestCase],
        targets: &[String],
        inputs: &JavaBytecodeInputs,
    ) -> CallGraphOutcome;
}
macro_rules! configured_provider {
    ($ty:ty,$label:literal) => {
        impl ConfiguredJavaProvider for $ty {
            fn label(&self) -> &str {
                $label
            }
            fn prepare_module(&self, module: &Path) -> Result<JavaBytecodeInputs, String> {
                self.prepare_module_inputs(module)
            }
            fn analyze_prepared(
                &self,
                root: &Path,
                tests: &[JavaTestCase],
                targets: &[String],
                inputs: &JavaBytecodeInputs,
            ) -> CallGraphOutcome {
                self.analyze_prepared_test_roots(root, tests, targets, inputs)
            }
        }
    };
}
configured_provider!(WalaJavaProvider, "WALA");
configured_provider!(JavaAdapterProvider, "SootUp");

pub fn analyze_configured_impact(
    project: &Path,
    baseline: &str,
    candidate: &str,
    changes: Vec<ChangedElement>,
    inputs: &ConfiguredJavaInputs,
    provider: Option<&dyn ConfiguredJavaProvider>,
    quiet: bool,
) -> anyhow::Result<TestImpactResult> {
    let mut result =
        TestImpactResult::new(baseline.into(), candidate.into(), changes, vec![], vec![]);
    result.diagnostics.extend(inputs.diagnostics.clone());
    if result.changed_elements.is_empty() {
        apply_configured_execution_scope(&mut result, inputs, true);
        return Ok(result);
    }
    let discovery_started = Instant::now();
    let mut tests = match discover_java_tests_in_checkout(project) {
        Ok(tests) => tests,
        Err(e) => {
            result.requires_full_test_run = true;
            result
                .diagnostics
                .push(format!("automatic test discovery failed: {e}"));
            vec![]
        }
    };
    for module in inputs.modules.values() {
        if module.tests.is_empty() {
            // An empty effective inventory cannot silently erase observable source tests.
            match discover_java_tests_in_roots(&module.root, &module.test_source_roots) {
                Ok(known) if !known.is_empty() => {
                    result.requires_full_test_run = true;
                    result.diagnostics.push(format!(
                        "empty configured test inventory contradicts source tests: {}",
                        module.root.display()
                    ));
                    tests.extend(known);
                    continue;
                }
                Err(error) => {
                    result.requires_full_test_run = true;
                    result.diagnostics.push(format!(
                        "configured test inventory verification failed: {error}"
                    ));
                    continue;
                }
                _ => {}
            }
        }
        tests.retain(|t| Path::new(&t.module_root) != module.root);
        tests.extend(module.tests.clone());
    }
    tests.sort_by(|a, b| {
        (&a.module_root, &a.class_name, &a.method_name, &a.descriptor).cmp(&(
            &b.module_root,
            &b.class_name,
            &b.method_name,
            &b.descriptor,
        ))
    });
    tests.dedup();
    result.diagnostics.push(format!(
        "timing: test discovery={}ms",
        discovery_started.elapsed().as_millis()
    ));
    let dependency_started = Instant::now();
    let graph = ModuleDependencyGraph::load_with_configured_inputs(project, inputs);
    result.diagnostics.push(format!(
        "timing: module dependency metadata={}ms",
        dependency_started.elapsed().as_millis()
    ));
    let graph = match graph {
        Ok(graph) if !inputs.requires_full_test_run() => graph,
        other => {
            let reason = match other {
                Err(e) => format!("module dependency metadata fallback: {e}"),
                _ => "configured manifest scope incomplete".into(),
            };
            select_broad(&mut result, tests, reason);
            apply_configured_execution_scope(&mut result, inputs, false);
            return Ok(result);
        }
    };
    let known_modules = graph.modules().collect::<std::collections::BTreeSet<_>>();
    // Drop tests from modules that the reactor (settings.gradle / root pom.xml) explicitly
    // declared as subprojects but that were intentionally excluded from the manifest.
    // Tests in builds that no reactor ever claimed are kept so the check below can
    // trigger a conservative full run for them.
    if inputs.module_inventory_complete && !inputs.modules.is_empty() {
        let reactor_modules = ModuleDependencyGraph::reactor_claimed_modules(project);
        tests.retain(|t| {
            let m = PathBuf::from(&t.module_root);
            inputs.modules.contains_key(&m) || !reactor_modules.contains(&m)
        });
    }
    if result.requires_full_test_run
        || tests
            .iter()
            .any(|t| !known_modules.contains(&PathBuf::from(&t.module_root)))
    {
        select_broad(
            &mut result,
            tests,
            "test owner or configured inventory lacks complete dependency/test evidence".into(),
        );
        apply_configured_execution_scope(&mut result, inputs, false);
        return Ok(result);
    }
    let mut by_module: BTreeMap<PathBuf, Vec<JavaTestCase>> = BTreeMap::new();
    for test in tests {
        by_module
            .entry(PathBuf::from(&test.module_root))
            .or_default()
            .push(test);
    }
    let mut prepared: BTreeMap<PathBuf, JavaBytecodeInputs> = BTreeMap::new();
    for (module, tests) in by_module {
        let relevant: Vec<_> = result
            .changed_elements
            .iter()
            .filter(|change| {
                let owner = Path::new(&change.module_root);
                let path = project.join(&change.source_path);
                let test_only = module_source_layout(owner, Some(inputs))
                    .test_roots
                    .iter()
                    .any(|r| path.starts_with(r));
                module.starts_with(owner)
                    || (!test_only && graph.consumers_of(owner).contains(&module))
            })
            .cloned()
            .collect();
        if relevant.is_empty() {
            result.unselected_tests.extend(tests);
            continue;
        }
        if relevant
            .iter()
            .any(|c| c.kind == ChangedElementKind::Module)
        {
            select_broad(
                &mut result,
                tests,
                "module-level change affects configured module or consumer".into(),
            );
            continue;
        }
        let Some(provider) = provider else {
            select_broad(&mut result, tests, "call-graph provider unavailable".into());
            continue;
        };
        let mut modules = graph.upstream_of(&module);
        modules.insert(module.clone());
        let layout = JavaSourceLayout {
            production_roots: modules
                .iter()
                .flat_map(|m| module_source_layout(m, Some(inputs)).production_roots)
                .collect(),
            test_roots: module_source_layout(&module, Some(inputs)).test_roots,
        };
        let extraction_started = Instant::now();
        let callables =
            crate::get_java_callables_in_roots_with_progress(&layout.production_roots, !quiet)?;
        result.diagnostics.push(format!(
            "timing: source extraction={}ms",
            extraction_started.elapsed().as_millis()
        ));
        if relevant.iter().any(|c| {
            callgraph_targets_in_roots(
                &layout.production_roots,
                std::slice::from_ref(c),
                &callables,
            )
            .is_empty()
        }) {
            select_broad(
                &mut result,
                tests,
                "unresolved changed source identity in configured roots".into(),
            );
            continue;
        }
        let targets = callgraph_targets_in_roots(&layout.production_roots, &relevant, &callables);
        let analysis_started = Instant::now();
        let mut failure = None;
        for root in &modules {
            if prepared.contains_key(root) {
                continue;
            }
            let value = match inputs.modules.get(root) {
                Some(m) => {
                    result.diagnostics.push(format!(
                        "artifact preparation skipped: supplied manifest for {}",
                        root.display()
                    ));
                    Ok(m.bytecode.clone())
                }
                None => provider.prepare_module(root),
            };
            match value {
                Ok(value) => {
                    prepared.insert(root.clone(), value);
                }
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            }
        }
        let mut outcome = if let Some(error) = failure {
            CallGraphOutcome {
                schema_version: 2,
                status: CallGraphStatus::Failed,
                provider_id: provider.label().into(),
                source_root: project.to_string_lossy().into(),
                algorithm: "cha".into(),
                diagnostics: vec![format!("artifact preparation failed: {error}")],
                reachability: vec![],
                edges: vec![],
            }
        } else {
            let refs = modules
                .iter()
                .filter_map(|m| prepared.get(m))
                .collect::<Vec<_>>();
            let bytecode =
                JavaBytecodeInputs::merge(&refs, &modules.iter().cloned().collect::<Vec<_>>());
            provider.analyze_prepared(project, &tests, &targets, &bytecode)
        };
        result.diagnostics.push(format!(
            "timing: {} analysis={}ms",
            provider.label(),
            analysis_started.elapsed().as_millis()
        ));
        let resolution =
            resolve_reachability_with_java_layout(&outcome, project, &callables, &tests, &layout);
        outcome.diagnostics.extend(resolution.diagnostics);
        let scoped = relevant
            .into_iter()
            .map(|mut c| {
                c.module_root = module.to_string_lossy().into();
                c
            })
            .collect();
        let selected = select_tests_with_reachability(
            scoped,
            tests,
            outcome,
            resolution.edges,
            &resolution.reachable_tests_by_callable,
        );
        result.selected_tests.extend(selected.selected_tests);
        result.unselected_tests.extend(selected.unselected_tests);
        result.diagnostics.extend(selected.diagnostics);
    }
    apply_configured_execution_scope(&mut result, inputs, true);
    Ok(result)
}

fn select_broad(result: &mut TestImpactResult, tests: Vec<JavaTestCase>, reason: String) {
    result.diagnostics.push(reason.clone());
    result
        .selected_tests
        .extend(tests.into_iter().map(|test| SelectedTest {
            test,
            reasons: vec![SelectionReason::ModuleFallback {
                diagnostic: reason.clone(),
            }],
        }));
}
