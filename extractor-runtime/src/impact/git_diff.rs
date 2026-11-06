use super::java_changes::map_java_range;
use models::{ChangedElement, ChangedElementKind};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
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
        if status.kind == 'M' || status.kind == 'A' {
            if status.candidate_path.ends_with(".java") {
                let ranges = candidate_ranges(&patch, &status.candidate_path);
                if !ranges.is_empty() {
                    for range in ranges {
                        changes.extend(map_java_range(
                            project_root,
                            &status.candidate_path,
                            range,
                        )?);
                    }
                    continue;
                }
            }
        }
        changes.push(module_change(project_root, &status.candidate_path));
    }
    Ok(deduplicate(changes))
}

/// Runs Git in the candidate checkout and returns UTF-8 standard output.
fn git_output(project_root: &Path, arguments: &[&str]) -> Result<String, ImpactError> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(project_root)
        .output()
        .map_err(|error| ImpactError::Git(error.to_string()))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(ImpactError::Git(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    }
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

/// Returns candidate-side line ranges for one file from a zero-context unified patch.
fn candidate_ranges(patch: &str, candidate_path: &str) -> Vec<std::ops::RangeInclusive<usize>> {
    let mut current_path = None;
    let mut ranges = Vec::new();
    for line in patch.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            current_path = Some(path);
        } else if current_path == Some(candidate_path) && line.starts_with("@@") {
            if let Some(range) = line.split(' ').nth(2).and_then(parse_candidate_range) {
                ranges.push(range);
            }
        }
    }
    ranges
}

/// Parses the `+start,count` side of one unified-diff hunk header.
fn parse_candidate_range(value: &str) -> Option<std::ops::RangeInclusive<usize>> {
    let value = value.strip_prefix('+')?;
    let (start, count) = value.split_once(',').unwrap_or((value, "1"));
    let start = start.parse::<usize>().ok()?;
    let count = count.parse::<usize>().ok()?;
    (count > 0).then_some(start..=start + count - 1)
}

/// Creates a module-level fallback element for a path that cannot be narrowed safely.
fn module_change(project_root: &Path, source_path: &str) -> ChangedElement {
    ChangedElement {
        module_root: find_module_root(project_root, source_path)
            .to_string_lossy()
            .into_owned(),
        source_path: source_path.to_owned(),
        kind: ChangedElementKind::Module,
        callable_signature: None,
    }
}

/// Finds the closest existing Maven module root for a changed or deleted path.
pub(super) fn find_module_root(project_root: &Path, source_path: &str) -> PathBuf {
    let mut current = project_root.join(source_path);
    if current.extension().is_some() {
        current.pop();
    }
    while current.starts_with(project_root) {
        if current.join("pom.xml").is_file() {
            return current;
        }
        if !current.pop() {
            break;
        }
    }
    project_root.to_path_buf()
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
