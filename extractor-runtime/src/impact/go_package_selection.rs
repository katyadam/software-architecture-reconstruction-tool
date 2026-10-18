//! Whole-binary selection. No absence of a callable edge is exclusion proof.
use super::go_selection::{GoSelectionDecision, select_go_tests, validated_graph_suggestions};
use crate::call_graph::go_native::{GoAnalysisResult, ValidatedGoPackageInventory};
use models::{go_impact::*, go_package_impact::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Debug)]
pub struct GoPackageSelectionDecision {
    pub tests: GoSelectionDecision,
    pub selected_execution_packages: Vec<GoExecutionPackageDecision>,
    pub unselected_execution_packages: Vec<GoExecutionPackageDecision>,
    pub execution_inventory_complete: bool,
}

fn package_decision(
    binary: &GoExecutionPackage,
    changed: &BTreeSet<String>,
    intersection: Vec<String>,
    reason: &str,
) -> GoExecutionPackageDecision {
    GoExecutionPackageDecision {
        package: binary.clone(),
        argument: if binary.relative_directory == "." {
            ".".into()
        } else {
            format!("./{}", binary.relative_directory)
        },
        changed_owners: changed.iter().cloned().collect(),
        dependency_intersection: intersection,
        reason: reason.into(),
    }
}

pub fn select_go_packages(
    changes: &[GoChangedElement],
    inventory: &GoInventoryOutcome,
    evidence: &ValidatedGoPackageInventory,
    graph: Option<&GoAnalysisResult>,
) -> GoPackageSelectionDecision {
    let mut d = GoPackageSelectionDecision {
        tests: select_go_tests(changes, inventory, graph),
        selected_execution_packages: vec![],
        unselected_execution_packages: vec![],
        execution_inventory_complete: true,
    };
    let mut changed = BTreeSet::new();
    let fallback = |mut d: GoPackageSelectionDecision, changed: &BTreeSet<String>, why: &str| {
        d.tests.diagnostics.push(why.into());
        d.selected_execution_packages = evidence
            .evidence()
            .execution_packages
            .iter()
            .map(|b| package_decision(b, changed, vec![], why))
            .collect();
        d.unselected_execution_packages.clear();
        d
    };
    if !evidence.matches_inventory(inventory) {
        return fallback(
            d,
            &changed,
            "whole-module fallback: package evidence was validated against a different inventory",
        );
    }
    for change in changes {
        if change.kind != GoChangeKind::Callable
            || change.target.as_ref().is_none_or(String::is_empty)
            || !change.diagnostics.is_empty()
            || Path::new(&change.source_path).is_absolute()
        {
            return fallback(
                d,
                &changed,
                "whole-module fallback: comparison is not entirely mapped body-only changes",
            );
        }
        let path = Path::new(&inventory.source_root)
            .join(&change.source_path)
            .to_string_lossy()
            .into_owned();
        let Some(owners) = evidence.owners().get(&path) else {
            return fallback(
                d,
                &changed,
                "whole-module fallback: changed source has no verified package owner",
            );
        };
        changed.extend(owners.iter().cloned());
    }
    if changes.is_empty() {
        d.execution_inventory_complete = false;
        return d;
    }
    let nodes: BTreeMap<_, _> = evidence
        .evidence()
        .packages
        .iter()
        .map(|p| (p.id.as_str(), p))
        .collect();
    let mut binaries: Vec<_> = evidence.evidence().execution_packages.iter().collect();
    binaries.sort_by(|a, b| a.import_path.cmp(&b.import_path));
    for binary in binaries {
        let mut todo = binary.roots.clone();
        let mut closure = BTreeSet::new();
        while let Some(id) = todo.pop() {
            if !closure.insert(id.clone()) {
                continue;
            }
            todo.extend(nodes[id.as_str()].imports.iter().cloned());
        }
        let intersection: Vec<_> = closure.intersection(&changed).cloned().collect();
        if intersection.is_empty() {
            d.unselected_execution_packages.push(package_decision(
                binary,
                &changed,
                intersection,
                "linked-binary dependency closure contains no changed source owner",
            ));
        } else {
            d.selected_execution_packages.push(package_decision(
                binary,
                &changed,
                intersection,
                "whole binary imports a changed source owner",
            ));
        }
    }
    let excluded: BTreeSet<_> = d
        .unselected_execution_packages
        .iter()
        .map(|p| p.package.import_path.as_str())
        .collect();
    if validated_graph_suggestions(changes, inventory, graph).is_some_and(|suggestions| {
        suggestions
            .iter()
            .any(|s| excluded.contains(s.test.execution_package.as_str()))
    }) {
        return fallback(
            d,
            &changed,
            "whole-module fallback: valid positive graph evidence contradicts a proposed package exclusion",
        );
    }
    d.tests.selected_tests.clear();
    d.tests.unselected_tests.clear();
    for test in &d.tests.discovered_tests {
        if excluded.contains(test.execution_package.as_str()) {
            d.tests.unselected_tests.push(test.clone());
        } else {
            d.tests.selected_tests.push(GoSelectedTest {
                test: test.clone(),
                reasons: vec![GoSelectionReason::PackageDependency {
                    execution_package: test.execution_package.clone(),
                    changed_owners: changed.iter().cloned().collect(),
                }],
            });
        }
    }
    d.tests
        .diagnostics
        .retain(|why| !why.starts_with("Go coverage is partial;"));
    d.tests.selection_mode = GoSelectionMode::ConservativePackages;
    d.tests.run_all_tests =
        !d.selected_execution_packages.is_empty() && d.unselected_execution_packages.is_empty();
    d
}
