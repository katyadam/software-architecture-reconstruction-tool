//! Git-to-source change mapping for Python test impact analysis.
//!
//! When all changed elements are callable-level and pyan3 produces a call graph, the tool
//! uses `CallGraphPrecise` mode to select only tests reachable from changed callables.
//! Otherwise it falls back to `ConservativeAll` (run every discovered test).

use super::{
    python_changes::{PythonFileDiff, PythonHunk, map_python_changes},
    python_graph::graph_suggestions,
    python_tests::discover_python_tests,
};
use crate::call_graph::pyan3::{Pyan3Provider, Pyan3Status};
use models::{
    call_graph::Language,
    python_impact::*,
};
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

#[derive(Debug, thiserror::Error)]
pub enum PythonImpactError {
    #[error("Invalid Python impact input: {0}")]
    Input(String),
    #[error("Python impact Git failure: {0}")]
    Git(String),
}

pub struct PythonImpactOptions {
    pub project_dir: PathBuf,
    pub baseline_revision: String,
    /// `None` or `Some(Duration::ZERO)` skips the pyan3 call-graph phase entirely.
    pub pyan3_timeout: Option<Duration>,
}

pub fn analyze_python_impact(
    options: &PythonImpactOptions,
) -> Result<PythonImpactReport, PythonImpactError> {
    let project_root = options
        .project_dir
        .canonicalize()
        .map_err(|e| PythonImpactError::Input(e.to_string()))?;

    if !project_root.join(".git").exists() {
        return Err(PythonImpactError::Input(
            "project-dir must be a Git checkout".into(),
        ));
    }

    let baseline_revision = resolve_revision(&project_root, &options.baseline_revision)?;
    let candidate_revision = resolve_revision(&project_root, "HEAD")?;

    eprintln!("python_impact_phase=git_diff");
    let diff_start = Instant::now();
    let files = read_python_diff(&project_root, &baseline_revision, &candidate_revision)?;
    let diff_ms = diff_start.elapsed().as_millis() as u64;

    if files.is_empty() {
        return Ok(no_changes_report(
            &project_root,
            baseline_revision,
            candidate_revision,
            diff_ms,
        ));
    }

    eprintln!("python_impact_phase=discovery");
    let disc_start = Instant::now();
    let discovered_tests = discover_python_tests(&project_root);
    let disc_ms = disc_start.elapsed().as_millis() as u64;

    eprintln!("python_impact_phase=source_mapping");
    let map_start = Instant::now();
    let changed_elements = map_python_changes(&project_root, &files);
    let map_ms = map_start.elapsed().as_millis() as u64;

    // --- optional call-graph phase ---
    let effective_timeout = options.pyan3_timeout.unwrap_or(Duration::ZERO);
    let (graph_suggested_tests, analysis_stage) = if effective_timeout.is_zero() {
        (
            vec![],
            skipped_stage("pyan3 call-graph phase skipped (timeout=0 or not configured)"),
        )
    } else {
        eprintln!("python_impact_phase=callgraph");
        let pyan3 = Pyan3Provider::new(effective_timeout);
        // Scope pyan3 to the service root containing the changed files rather than the
        // whole project root. Multi-service monorepos cause pyan3 to drop edges when
        // given too many files; scoping to the relevant service preserves method-level nodes.
        let pyan3_scope = find_python_scope(&project_root, &changed_elements);
        let result = pyan3.analyze_project(&pyan3_scope);
        let duration_ms = result.duration_ms;
        let (status, mut diag) = match result.status {
            Pyan3Status::Ok => (PythonStageStatus::Ok, result.diagnostics),
            Pyan3Status::NotInstalled => {
                let mut d = result.diagnostics;
                d.insert(0, "pyan3 not installed; graph phase skipped".into());
                (PythonStageStatus::Skipped, d)
            }
            Pyan3Status::Failed => {
                let mut d = result.diagnostics;
                d.insert(0, "pyan3 failed; falling back to conservative selection".into());
                (PythonStageStatus::Failed, d)
            }
        };
        let targets: Vec<&str> = changed_elements
            .iter()
            .filter_map(|e| e.target.as_deref())
            .collect();
        let suggestions = if matches!(status, PythonStageStatus::Ok) && !targets.is_empty() {
            let s = graph_suggestions(&targets, &discovered_tests, &result.edges);
            diag.push(format!(
                "pyan3 suggested {}/{} tests reachable from changed callables",
                s.len(),
                discovered_tests.len()
            ));
            s
        } else {
            vec![]
        };
        (
            suggestions,
            PythonStageReport {
                status,
                diagnostics: diag,
                duration_ms,
            },
        )
    };

    // --- selection phase ---
    eprintln!("python_impact_phase=selection");
    let (selected_tests, unselected_tests, run_all_tests, selection_mode) =
        if changed_elements.is_empty() {
            (vec![], vec![], false, PythonSelectionMode::NoChanges)
        } else {
            // Use CallGraphPrecise when pyan3 succeeded and the graph actually excludes
            // some tests (strict subset reachable). This is safe even when some changed
            // elements are module-level (e.g. a new import or logger declaration) because
            // pyan3 does not model module-level data flow; the caller graph is still the
            // right signal for which test classes transitively exercise the changed callables.
            let can_use_graph = matches!(analysis_stage.status, PythonStageStatus::Ok)
                && !graph_suggested_tests.is_empty()
                && graph_suggested_tests.len() < discovered_tests.len();

            if can_use_graph {
                let suggested_set: std::collections::HashSet<_> = graph_suggested_tests
                    .iter()
                    .map(|s| &s.test.qualified_name)
                    .collect();
                let selected = graph_suggested_tests
                    .iter()
                    .map(|s| {
                        let reasons = s
                            .changed_targets
                            .iter()
                            .map(|t| PythonSelectionReason::ChangedCallable {
                                target: t.clone(),
                            })
                            .collect();
                        PythonSelectedTest {
                            test: s.test.clone(),
                            reasons,
                        }
                    })
                    .collect();
                let unselected = discovered_tests
                    .iter()
                    .filter(|t| !suggested_set.contains(&t.qualified_name))
                    .cloned()
                    .collect();
                (selected, unselected, false, PythonSelectionMode::CallGraphPrecise)
            } else {
                let conservative_diag =
                    "Python coverage is conservative; run every discovered test".to_string();
                let selected = discovered_tests
                    .iter()
                    .map(|t| {
                        let mut reasons =
                            vec![PythonSelectionReason::ConservativeFallback {
                                diagnostic: conservative_diag.clone(),
                            }];
                        // Annotate with graph-suggested changed callables (advisory).
                        if let Some(suggestion) =
                            graph_suggested_tests.iter().find(|s| &s.test == t)
                        {
                            for target in &suggestion.changed_targets {
                                let r = PythonSelectionReason::ChangedCallable {
                                    target: target.clone(),
                                };
                                if !reasons.contains(&r) {
                                    reasons.push(r);
                                }
                            }
                        } else {
                            // Fall back to the module-level changed callable list.
                            for elem in &changed_elements {
                                if let Some(target) = &elem.target {
                                    let r = PythonSelectionReason::ChangedCallable {
                                        target: target.clone(),
                                    };
                                    if !reasons.contains(&r) {
                                        reasons.push(r);
                                    }
                                }
                            }
                        }
                        PythonSelectedTest {
                            test: t.clone(),
                            reasons,
                        }
                    })
                    .collect();
                (selected, vec![], true, PythonSelectionMode::ConservativeAll)
            }
        };

    Ok(PythonImpactReport {
        schema_version: 1,
        language: Language::Python,
        project_root: project_root.to_string_lossy().into_owned(),
        baseline_revision,
        candidate_revision,
        changed_elements,
        discovered_tests,
        test_inventory_complete: true,
        graph_suggested_tests,
        selected_tests,
        unselected_tests,
        selection_mode,
        run_all_tests,
        discovery: PythonStageReport {
            status: PythonStageStatus::Ok,
            diagnostics: vec![format!("timing: discovery={disc_ms}ms")],
            duration_ms: disc_ms,
        },
        mapping: PythonStageReport {
            status: PythonStageStatus::Ok,
            diagnostics: vec![format!("timing: mapping={map_ms}ms")],
            duration_ms: map_ms,
        },
        analysis: analysis_stage,
        diagnostics: vec![
            format!("timing: git_diff={diff_ms}ms"),
            "Python coverage is conservative; run every discovered test".into(),
        ],
    })
}

fn no_changes_report(
    project_root: &Path,
    baseline_revision: String,
    candidate_revision: String,
    diff_ms: u64,
) -> PythonImpactReport {
    PythonImpactReport {
        schema_version: 1,
        language: Language::Python,
        project_root: project_root.to_string_lossy().into_owned(),
        baseline_revision,
        candidate_revision,
        changed_elements: vec![],
        discovered_tests: vec![],
        test_inventory_complete: true,
        graph_suggested_tests: vec![],
        selected_tests: vec![],
        unselected_tests: vec![],
        selection_mode: PythonSelectionMode::NoChanges,
        run_all_tests: false,
        discovery: skipped_stage("no changes"),
        mapping: skipped_stage("no changes"),
        analysis: skipped_stage("no changes"),
        diagnostics: vec![format!("timing: git_diff={diff_ms}ms")],
    }
}

fn skipped_stage(why: &str) -> PythonStageReport {
    PythonStageReport {
        status: PythonStageStatus::Skipped,
        diagnostics: vec![why.into()],
        duration_ms: 0,
    }
}

/// Finds the narrowest directory containing all changed Python files that also looks like
/// a Python project root (has `manage.py`, `setup.py`, `pyproject.toml`, or `setup.cfg`).
///
/// Falls back to `project_root` if no marker is found or the changed elements have no
/// source paths. This scopes pyan3 to a single service in a monorepo and prevents
/// edge degradation from passing too many unrelated files.
fn find_python_scope(project_root: &Path, changed_elements: &[PythonChangedElement]) -> PathBuf {
    const MARKERS: &[&str] = &["manage.py", "setup.py", "pyproject.toml", "setup.cfg"];

    // Collect the directories of all changed source files.
    let dirs: Vec<PathBuf> = changed_elements
        .iter()
        .filter_map(|e| {
            let path = e.source_path.as_str();
            path.rsplit_once('/').map(|(d, _)| d.to_owned())
        })
        .filter_map(|d| {
            let abs = project_root.join(&d);
            if abs.is_dir() { Some(abs) } else { None }
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    if dirs.is_empty() {
        return project_root.to_path_buf();
    }

    // Find the deepest common ancestor of all change directories.
    let mut common: &Path = dirs[0].as_path();
    for d in &dirs[1..] {
        while !d.starts_with(common) {
            match common.parent() {
                Some(p) if p != common => common = p,
                _ => return project_root.to_path_buf(),
            }
        }
    }

    // Walk from the common ancestor up to project_root, stopping at the first Python root marker.
    let mut candidate = common;
    loop {
        if MARKERS.iter().any(|m| candidate.join(m).exists()) {
            return candidate.to_path_buf();
        }
        match candidate.parent() {
            Some(p) if p != candidate && p.starts_with(project_root) => {
                candidate = p;
            }
            _ => break,
        }
    }

    // No marker found — check if project_root itself has a marker.
    if MARKERS.iter().any(|m| project_root.join(m).exists()) {
        return project_root.to_path_buf();
    }

    // Last resort: use the common ancestor directory directly.
    common.to_path_buf()
}

/// Parses `start,count` from one side of a unified-diff hunk header.
fn parse_hunk_side(value: &str) -> Option<(u32, u32)> {
    let (s, c) = value.split_once(',').unwrap_or((value, "1"));
    Some((s.parse().ok()?, c.parse().ok()?))
}

fn resolve_revision(root: &Path, rev: &str) -> Result<String, PythonImpactError> {
    if rev.is_empty() || rev.starts_with('-') {
        return Err(PythonImpactError::Input(
            "revision must not be empty or a Git option".into(),
        ));
    }
    let out = Command::new("git")
        .args([
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{rev}^{{commit}}"),
        ])
        .current_dir(root)
        .output()
        .map_err(|e| PythonImpactError::Git(e.to_string()))?;
    if !out.status.success() {
        return Err(PythonImpactError::Git(
            String::from_utf8_lossy(&out.stderr).trim().into(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().into())
}

fn git_bytes(root: &Path, args: &[&str]) -> Result<Vec<u8>, PythonImpactError> {
    let out = Command::new("git")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| PythonImpactError::Git(e.to_string()))?;
    if !out.status.success() {
        return Err(PythonImpactError::Git(
            String::from_utf8_lossy(&out.stderr).trim().into(),
        ));
    }
    Ok(out.stdout)
}

/// Reads a name-status diff and parses it into `PythonFileDiff` records with hunk data.
fn read_python_diff(
    root: &Path,
    baseline: &str,
    candidate: &str,
) -> Result<Vec<PythonFileDiff>, PythonImpactError> {
    let status_raw = git_bytes(
        root,
        &[
            "diff",
            "--name-status",
            "-z",
            "--find-renames",
            "--no-ext-diff",
            "--no-textconv",
            baseline,
            candidate,
            "--",
        ],
    )?;
    let mut fields = status_raw.split(|b| *b == 0).peekable();
    let mut diffs = vec![];
    while let Some(raw) = fields.next() {
        if raw.is_empty() {
            break;
        }
        let status = String::from_utf8_lossy(raw).into_owned();
        let kind = status.chars().next().unwrap_or('?');
        if !"MADRTC".contains(kind) {
            return Err(PythonImpactError::Git(format!(
                "unsupported diff status: {status}"
            )));
        }
        let path = |f: &mut std::iter::Peekable<_>| -> Result<String, PythonImpactError> {
            let p = f
                .next()
                .ok_or_else(|| PythonImpactError::Git("missing path".into()))?;
            Ok(String::from_utf8_lossy(p).into_owned())
        };
        let first = path(&mut fields)?;
        let (old, new) = match kind {
            'A' => (None, Some(first)),
            'D' => (Some(first), None),
            'R' | 'C' => (Some(first), Some(path(&mut fields)?)),
            _ => (Some(first.clone()), Some(first)),
        };
        let candidate_path = new.as_deref().unwrap_or(old.as_deref().unwrap_or(""));
        let mut hunks = vec![];
        if kind == 'M' && candidate_path.ends_with(".py") {
            let patch = git_bytes(
                root,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-color",
                    "--unified=0",
                    baseline,
                    candidate,
                    "--",
                    candidate_path,
                ],
            )?;
            for line in String::from_utf8_lossy(&patch)
                .lines()
                .filter(|l| l.starts_with("@@ "))
            {
                let parts: Vec<_> = line.split(' ').collect();
                let old = parts.get(1).and_then(|s| s.strip_prefix('-')).and_then(parse_hunk_side);
                let new = parts.get(2).and_then(|s| s.strip_prefix('+')).and_then(parse_hunk_side);
                if let (Some((os, oc)), Some((ns, nc))) = (old, new) {
                    hunks.push(PythonHunk {
                        old_start: os,
                        old_count: oc,
                        new_start: ns,
                        new_count: nc,
                    });
                }
            }
        }
        // Fetch baseline source only when a deletion hunk needs it.
        let old_source = if hunks.iter().any(|h| h.new_count == 0 && h.old_count > 0) {
            git_bytes(root, &["show", &format!("{baseline}:{candidate_path}")])
                .ok()
                .and_then(|b| String::from_utf8(b).ok())
        } else {
            None
        };
        diffs.push(PythonFileDiff {
            status,
            old_path: old,
            new_path: new,
            hunks,
            old_source,
        });
    }
    Ok(diffs)
}
