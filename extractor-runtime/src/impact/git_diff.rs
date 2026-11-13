use super::{
    ModuleCatalog,
    inert::is_inert_path,
    java_changes::{map_java_range, map_java_range_in_source},
};
use models::{ChangedElement, ChangedElementKind};
use std::{ops::RangeInclusive, path::Path, process::Command};
use thiserror::Error;

/// Reports failures while obtaining or interpreting a Git candidate diff.
#[derive(Debug, Error)]
pub enum ImpactError {
    /// Git could not be started or returned a non-success status.
    #[error("Git diff failed: {0}")]
    Git(String),
    /// A candidate source file could not be read for Tree-sitter ownership mapping.
    #[error("Candidate source could not be read: {0}")]
    Source(String),
}

/// Maps changes from `baseline_revision` through the checked-out candidate `HEAD` to impact elements.
pub fn analyze_changes(
    project_root: &Path,
    baseline_revision: &str,
) -> Result<Vec<ChangedElement>, ImpactError> {
    analyze_changes_with_catalog(
        project_root,
        baseline_revision,
        &ModuleCatalog::load(project_root),
    )
}

pub fn analyze_changes_with_catalog(
    project_root: &Path,
    baseline_revision: &str,
    catalog: &ModuleCatalog,
) -> Result<Vec<ChangedElement>, ImpactError> {
    let statuses = git_output(
        project_root,
        &[
            "diff",
            "--name-status",
            "--find-renames",
            baseline_revision,
            "HEAD",
        ],
    )?;
    let patch = git_output(
        project_root,
        &[
            "diff",
            "--unified=0",
            "--find-renames",
            baseline_revision,
            "HEAD",
        ],
    )?;
    let mut changes = Vec::new();

    for status in parse_statuses(&statuses) {
        if is_inert_path(&status.candidate_path) {
            continue;
        }
        if (status.kind == 'M' || status.kind == 'A')
            && status.candidate_path.ends_with(".java")
            && catalog.can_narrow_source(&status.candidate_path)
        {
            let hunks = diff_hunks(&patch, &status.candidate_path);
            if !hunks.is_empty() {
                let module_root = catalog
                    .owner_for(&status.candidate_path)
                    .to_string_lossy()
                    .into_owned();
                // Fetch baseline source only when at least one hunk is a pure deletion.
                let old_source: Option<String> =
                    if hunks.iter().any(|h| h.new.is_none() && h.old.is_some()) {
                        git_output(
                            project_root,
                            &[
                                "show",
                                &format!("{baseline_revision}:{}", status.candidate_path),
                            ],
                        )
                        .ok()
                    } else {
                        None
                    };
                let mut file_changes: Vec<ChangedElement> = Vec::new();
                let mut any_unmapped = false;
                for hunk in &hunks {
                    match (&hunk.old, &hunk.new) {
                        (_, Some(new_range)) => {
                            file_changes.extend(map_java_range(
                                project_root,
                                catalog,
                                &status.candidate_path,
                                new_range.clone(),
                            )?);
                        }
                        (Some(old_range), None) => match &old_source {
                            Some(src) => {
                                file_changes.extend(map_java_range_in_source(
                                    src,
                                    &module_root,
                                    &status.candidate_path,
                                    old_range.clone(),
                                ));
                            }
                            None => {
                                any_unmapped = true;
                                break;
                            }
                        },
                        (None, None) => {}
                    }
                }
                if !any_unmapped {
                    changes.extend(file_changes);
                    continue;
                }
            }
        }
        changes.push(module_change(catalog, &status.candidate_path));
    }
    Ok(deduplicate(changes))
}

/// Runs Git in `root` with the given arguments, returning raw stdout bytes.
/// Sets `GIT_LITERAL_PATHSPECS=1` so path arguments are never glob-expanded.
/// Returns `Err(message)` on launch failure or non-zero exit.
pub(super) fn run_git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("git")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().into());
    }
    Ok(out.stdout)
}

/// Resolves `rev` to a full commit SHA using `git rev-parse --verify`.
/// Returns `Err(message)` for empty/option-like revisions or git failures.
pub(super) fn resolve_revision(root: &Path, rev: &str) -> Result<String, String> {
    if rev.is_empty() || rev.starts_with('-') {
        return Err("revision must not be empty or a Git option".into());
    }
    let raw = run_git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{rev}^{{commit}}"),
        ],
    )?;
    String::from_utf8(raw)
        .map_err(|_| "non-UTF8 revision output".into())
        .map(|s| s.trim().into())
}

/// Runs Git in the candidate checkout and returns UTF-8 standard output.
fn git_output(project_root: &Path, arguments: &[&str]) -> Result<String, ImpactError> {
    run_git(project_root, arguments)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .map_err(ImpactError::Git)
}

/// Holds one Git name-status record and the candidate path used for impact ownership.
struct FileStatus {
    kind: char,
    candidate_path: String,
}

/// Parses ordinary and rename Git name-status records.
fn parse_statuses(output: &str) -> Vec<FileStatus> {
    output
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            let status = fields.first()?.chars().next()?;
            let candidate_path = if status == 'R' || status == 'C' {
                fields.get(2)?
            } else {
                fields.get(1)?
            };
            Some(FileStatus {
                kind: status,
                candidate_path: (*candidate_path).to_owned(),
            })
        })
        .collect()
}

/// One hunk from a unified diff: old-side range and new-side range.
/// A `None` range means that side has zero lines (pure addition or pure deletion).
struct DiffHunk {
    old: Option<RangeInclusive<usize>>,
    new: Option<RangeInclusive<usize>>,
}

/// Returns old/new range pairs for every hunk that touches `candidate_path`.
fn diff_hunks(patch: &str, candidate_path: &str) -> Vec<DiffHunk> {
    let mut current_path = None;
    let mut hunks = Vec::new();
    for line in patch.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            current_path = Some(path);
        } else if current_path == Some(candidate_path) && line.starts_with("@@") {
            let parts: Vec<_> = line.split(' ').collect();
            let old = parts
                .get(1)
                .and_then(|s| s.strip_prefix('-'))
                .and_then(parse_range);
            let new = parts
                .get(2)
                .and_then(|s| s.strip_prefix('+'))
                .and_then(parse_range);
            hunks.push(DiffHunk { old, new });
        }
    }
    hunks
}

/// Parses a `start,count` hunk side into an inclusive line range.
/// Returns `None` when `count` is zero (no lines on that side).
fn parse_range(value: &str) -> Option<RangeInclusive<usize>> {
    let (start, count) = parse_hunk_side(value)?;
    (count > 0).then_some(start as usize..=start as usize + count as usize - 1)
}

/// Parses `start,count` from one side of a unified-diff hunk header (e.g. `5,3`).
/// A bare number without a comma is treated as `start,1`.
/// Returns `None` on any parse failure.
pub(super) fn parse_hunk_side(value: &str) -> Option<(u32, u32)> {
    let (s, c) = value.split_once(',').unwrap_or((value, "1"));
    Some((s.parse().ok()?, c.parse().ok()?))
}

/// Creates a module-level fallback element for a path that cannot be narrowed safely.
fn module_change(catalog: &ModuleCatalog, source_path: &str) -> ChangedElement {
    ChangedElement {
        module_root: catalog
            .owner_for(source_path)
            .to_string_lossy()
            .into_owned(),
        source_path: source_path.to_owned(),
        kind: ChangedElementKind::Module,
        callable_signature: None,
    }
}

/// Removes duplicate elements caused by overlapping diff hunks.
fn deduplicate(changes: Vec<ChangedElement>) -> Vec<ChangedElement> {
    changes.into_iter().fold(Vec::new(), |mut unique, change| {
        if !unique.contains(&change) {
            unique.push(change);
        }
        unique
    })
}
