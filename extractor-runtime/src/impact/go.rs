//! Native Go impact analysis, separate from the Java command and identities.
pub use super::go_changes::map_go_changes;
pub use super::go_git::{GoFileDiff, GoGitDiff, GoHunk, read_go_diff};
pub use super::go_selection::{GoSelectionDecision, select_go_tests};
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
    if !diff.files.is_empty() {
        eprintln!("go_impact_phase=discovery");
        let start = Instant::now();
        inventory = provider.discover(&diff.module_root);
        discovery = stage(inventory.status, inventory.diagnostics.clone(), start);
        for path in &inventory.source_files {
            let rel = std::path::Path::new(path)
                .strip_prefix(&diff.module_root)
                .map_err(|_| GoImpactError::Input("inventory source outside module".into()))?;
            let relative = rel
                .to_str()
                .ok_or_else(|| GoImpactError::Input("non-UTF8 active Go source".into()))?;
            let committed = super::go_git::git(
                &diff.module_root,
                &[
                    "cat-file",
                    "blob",
                    &format!("{}:{relative}", diff.candidate_revision),
                ],
            )
            .map_err(|_| {
                GoImpactError::Input(format!(
                    "active Go source is not in candidate commit: {relative}"
                ))
            })?;
            let actual = std::fs::read(path).map_err(|e| GoImpactError::Input(e.to_string()))?;
            if committed != actual {
                return Err(GoImpactError::Input(format!(
                    "active source differs from committed HEAD: {relative}"
                )));
            }
        }
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
    let decision = select_go_tests(&changes, &inventory, graph.as_ref());
    let mut diagnostics = decision.diagnostics;
    for (label, s) in [
        ("discovery", &discovery),
        ("mapping", &mapping),
        ("analysis", &analysis),
    ] {
        diagnostics.extend(s.diagnostics.iter().map(|d| format!("{label}: {d}")));
    }
    Ok(GoImpactReport {
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
    })
}
