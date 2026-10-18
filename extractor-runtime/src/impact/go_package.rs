use super::{
    go::{GoImpactError, GoImpactOptions, analyze_go_impact_core},
    go_package_selection::GoPackageSelectionDecision,
};
use crate::call_graph::go_native::ValidatedGoPackageInventory;
use models::{go_impact::*, go_package_impact::*};

pub fn analyze_go_package_impact(
    options: &GoImpactOptions,
    scope: GoPackageTestScope,
) -> Result<GoPackageImpactReport, GoImpactError> {
    analyze_go_impact_core(options, Some(scope))?
        .1
        .ok_or_else(|| GoImpactError::Output("missing requested package report".into()))
}

pub(super) fn package_report(
    mut base: GoImpactReport,
    scope: GoPackageTestScope,
    stage: GoStageReport,
    evidence: Option<&ValidatedGoPackageInventory>,
    decision: Option<GoPackageSelectionDecision>,
) -> GoPackageImpactReport {
    base.schema_version = 2;
    let mut report = GoPackageImpactReport {
        base,
        requested_policy: GoSelectionPolicy::Packages,
        declared_test_scope: scope,
        effective_build_context: None,
        package_evidence: stage,
        execution_inventory_complete: false,
        discovered_execution_packages: vec![],
        selected_execution_packages: vec![],
        unselected_execution_packages: vec![],
    };
    if let (Some(e), Some(d)) = (evidence, decision) {
        report.execution_inventory_complete = d.execution_inventory_complete;
        report.effective_build_context = e.evidence().effective_build_context.clone();
        report.discovered_execution_packages = e.evidence().execution_packages.clone();
        report.selected_execution_packages = d.selected_execution_packages;
        report.unselected_execution_packages = d.unselected_execution_packages;
        report.base.selected_tests = d.tests.selected_tests;
        report.base.unselected_tests = d.tests.unselected_tests;
        report.base.selection_mode = d.tests.selection_mode;
        report.base.run_all_tests = d.tests.run_all_tests;
        report
            .base
            .diagnostics
            .retain(|why| !why.starts_with("Go coverage is partial;"));
        for diagnostic in d.tests.diagnostics {
            if !report.base.diagnostics.contains(&diagnostic) {
                report.base.diagnostics.push(diagnostic);
            }
        }
    }
    report.base.diagnostics.extend(
        report
            .package_evidence
            .diagnostics
            .iter()
            .map(|d| format!("package evidence: {d}")),
    );
    report
}
