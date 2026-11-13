//! Native Go impact analysis, separate from the Java command and identities.
pub use super::go_changes::map_go_changes;
pub use super::go_git::{GoFileDiff, GoGitDiff, GoHunk, read_go_diff};
pub use super::go_package::analyze_go_package_impact;
pub use super::go_selection::{GoSelectionDecision, select_go_tests};
use crate::call_graph::go_native::validate_package_inventory;
use models::go_package_impact::*;
#[derive(Debug, thiserror::Error)]
pub enum GoImpactError {
    #[error("Invalid Go impact input: {0}")]
    Input(String),
    #[error("Go impact Git failure: {0}")]
    Git(String),
    #[error("Go impact output failure: {0}")]
    Output(String),
}

use crate::call_graph::go_native::GoNativeProvider;
use models::{
    call_graph::{CallGraphStatus, Language},
    go_impact::*,
};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    time::{Duration, Instant},
};
pub struct GoImpactOptions {
    pub project_dir: PathBuf,
    pub baseline_revision: String,
    pub go_callgraph_binary: PathBuf,
    pub build_tags: String,
    pub analysis_timeout: Duration,
}

fn stage(status: GoStageStatus, diagnostics: Vec<String>, start: Instant) -> GoStageReport {
    GoStageReport {
        status,
        diagnostics,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}
fn skipped(why: &str) -> GoStageReport {
    GoStageReport {
        status: GoStageStatus::Skipped,
        diagnostics: vec![why.into()],
        duration_ms: 0,
    }
}

pub fn analyze_go_impact(options: &GoImpactOptions) -> Result<GoImpactReport, GoImpactError> {
    analyze_go_impact_core(options, None).map(|(base, _)| base)
}
pub(super) fn analyze_go_impact_core(
    options: &GoImpactOptions,
    package_scope: Option<GoPackageTestScope>,
) -> Result<(GoImpactReport, Option<GoPackageImpactReport>), GoImpactError> {
    if !cfg!(unix) || options.analysis_timeout.is_zero() {
        return Err(GoImpactError::Input(
            "Unix supervision and positive analysis timeout required".into(),
        ));
    }
    let binary = options
        .go_callgraph_binary
        .canonicalize()
        .map_err(|e| GoImpactError::Input(format!("prebuilt Go executable: {e}")))?;
    if !binary.is_file() {
        return Err(GoImpactError::Input(
            "go-callgraph-binary must be a file".into(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(&binary)
            .map_err(|e| GoImpactError::Input(e.to_string()))?
            .permissions()
            .mode()
            & 0o111
            == 0
        {
            return Err(GoImpactError::Input(
                "go-callgraph-binary is not executable".into(),
            ));
        }
    }
    eprintln!("go_impact_phase=git_diff");
    let diff = read_go_diff(&options.project_dir, &options.baseline_revision)?;
    let provider = GoNativeProvider::new(binary, options.analysis_timeout)
        .with_build_tags(&options.build_tags);
    let mut inventory = GoInventoryOutcome {
        schema_version: 1,
        provider_id: "go-native".into(),
        status: GoStageStatus::Skipped,
        source_root: diff.module_root.to_string_lossy().into(),
        source_files: vec![],
        tests: vec![],
        declarations: vec![],
        test_inventory_complete: false,
        scope: None,
        diagnostics: vec![],
    };
    let mut discovery = skipped("no changes");
    let mut mapping = skipped("no changes");
    let mut analysis = skipped("no changes");
    let mut graph = None;
    let mut changes = vec![];
    let mut package_inventory = None;
    let mut package_stage = skipped("package evidence not requested or comparison ineligible");
    if !diff.files.is_empty() {
        eprintln!("go_impact_phase=discovery");
        let start = Instant::now();
        inventory = provider.discover(&diff.module_root);
        discovery = stage(inventory.status, inventory.diagnostics.clone(), start);
        super::go_git::verify_committed_sources(&diff, &inventory.source_files)?;
        eprintln!("go_impact_phase=source_mapping");
        let start = Instant::now();
        let mut inputs = vec![];
        let mut diagnostics = vec![];
        for file in &diff.files {
            for (label, path, blob) in [
                ("baseline", &file.old_path, &file.old_source),
                ("candidate", &file.new_path, &file.new_source),
            ] {
                if let (Some(path), Some(blob)) = (path, blob) {
                    if path.ends_with(".go") {
                        match String::from_utf8(blob.clone()) {
                            Ok(source) => inputs.push(GoSourceInput {
                                source_path: format!("{label}:{path}"),
                                source,
                            }),
                            Err(_) => diagnostics.push(format!(
                                "non-UTF8 Go source requires fallback: {label}:{path}"
                            )),
                        }
                    }
                }
            }
        }
        let descriptions = if inputs.is_empty() {
            GoDescriptionOutcome {
                schema_version: 1,
                provider_id: "go-native".into(),
                status: GoStageStatus::Ok,
                files: vec![],
                diagnostics: vec![],
            }
        } else {
            provider.describe_sources(&inputs)
        };
        diagnostics.extend(descriptions.diagnostics.clone());
        for file in &descriptions.files {
            diagnostics.extend(
                file.diagnostics
                    .iter()
                    .map(|d| format!("{}: {d}", file.source_path)),
            );
        }
        changes = map_go_changes(&diff, &inventory, &descriptions);
        mapping = stage(
            if diagnostics.is_empty() {
                GoStageStatus::Ok
            } else {
                GoStageStatus::Failed
            },
            diagnostics,
            start,
        );
        if package_scope.is_some()
            && inventory.status == GoStageStatus::Ok
            && inventory.test_inventory_complete
            && mapping.status == GoStageStatus::Ok
            && !changes.is_empty()
            && changes.iter().all(|c| {
                c.kind == GoChangeKind::Callable && c.target.is_some() && c.diagnostics.is_empty()
            })
            && diff.files.iter().all(|f| {
                f.status == "M"
                    && f.old_source
                        .as_deref()
                        .zip(f.new_source.as_deref())
                        .is_some_and(|(old, new)| {
                            super::go_package_changes::only_function_bodies_changed(old, new)
                        })
            })
        {
            eprintln!("go_impact_phase=package_inventory");
            let start = Instant::now();
            let out = provider.discover_packages(&diff.module_root);
            let mut diagnostics = out.diagnostics.clone();
            diagnostics.extend(
                out.blockers
                    .iter()
                    .map(|b| format!("package blocker {}: {}", b.code, b.diagnostic)),
            );
            match validate_package_inventory(&diff.module_root, &inventory, &out) {
                Ok(valid) => {
                    let files = valid.owners().keys().cloned().collect::<Vec<_>>();
                    super::go_git::verify_committed_sources(&diff, &files)?;
                    package_inventory = Some(valid);
                    package_stage = stage(GoStageStatus::Ok, diagnostics, start);
                }
                Err(why) => {
                    diagnostics.push(why);
                    package_stage = stage(GoStageStatus::Failed, diagnostics, start);
                }
            }
        }
        let targets: Vec<_> = changes
            .iter()
            .filter_map(|c| c.target.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if inventory.test_inventory_complete && !targets.is_empty() {
            eprintln!("go_impact_phase=callgraph");
            let start = Instant::now();
            let out = provider.analyze_targets(&diff.module_root, &targets);
            analysis = stage(
                if matches!(
                    out.graph.status,
                    CallGraphStatus::Ok | CallGraphStatus::NoEntrypoints
                ) {
                    GoStageStatus::Ok
                } else {
                    GoStageStatus::Failed
                },
                out.graph.diagnostics.clone(),
                start,
            );
            graph = Some(out);
        } else {
            analysis = skipped(if targets.is_empty() {
                "module-only or unmapped changes; graph construction unnecessary"
            } else {
                "incomplete test inventory; graph construction skipped"
            });
        }
    }
    // Detect edits/commits while subprocesses were running; never publish them as
    // an analysis of the earlier candidate commit.
    super::go_git::check_clean(&diff.module_root)?;
    if super::go_git::revision(&diff.module_root, "HEAD")? != diff.candidate_revision {
        return Err(GoImpactError::Input(
            "candidate HEAD changed during analysis".into(),
        ));
    }
    eprintln!("go_impact_phase=selection");
    let package_decision = package_inventory.as_ref().map(|e| {
        super::go_package_selection::select_go_packages(&changes, &inventory, e, graph.as_ref())
    });
    let decision = select_go_tests(&changes, &inventory, graph.as_ref());
    let mut diagnostics = decision.diagnostics;
    for (label, s) in [
        ("discovery", &discovery),
        ("mapping", &mapping),
        ("analysis", &analysis),
    ] {
        diagnostics.extend(s.diagnostics.iter().map(|d| format!("{label}: {d}")));
    }
    let base = GoImpactReport {
        schema_version: 1,
        language: Language::Go,
        module_root: diff.module_root.to_string_lossy().into(),
        baseline_revision: diff.baseline_revision,
        candidate_revision: diff.candidate_revision,
        changed_elements: changes,
        discovered_tests: decision.discovered_tests,
        test_inventory_complete: decision.test_inventory_complete,
        graph_suggested_tests: decision.graph_suggested_tests,
        selected_tests: decision.selected_tests,
        unselected_tests: decision.unselected_tests,
        selection_mode: decision.selection_mode,
        run_all_tests: decision.run_all_tests,
        discovery,
        mapping,
        analysis,
        graph: graph.map(|g| g.graph),
        scope: inventory.scope,
        diagnostics,
    };
    let package = package_scope.map(|scope| {
        super::go_package::package_report(
            base.clone(),
            scope,
            package_stage,
            package_inventory.as_ref(),
            package_decision,
        )
    });
    Ok((base, package))
}
