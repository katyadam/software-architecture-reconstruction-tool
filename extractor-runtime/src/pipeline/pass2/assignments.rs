use std::collections::HashMap;

use models::{Assignment, AssignmentKey, Scope, ir::project::TypedFileRecord};

/// Build a map of all global-scope assignments across all files.
/// Used as a cross-file fallback when per-file assignment maps lack a variable.
pub fn build_cross_file_globals(files: &[TypedFileRecord]) -> HashMap<AssignmentKey, Assignment> {
    let mut globals = HashMap::new();
    for file in files {
        for (key, assignment) in &file.assignments {
            if matches!(key.scope, Scope::Global) {
                globals
                    .entry(key.clone())
                    .or_insert_with(|| assignment.clone());
            }
        }
    }
    globals
}

/// Merge file-local assignments with cross-file globals; local entries take priority.
pub fn merged_assignments(
    file_assignments: &HashMap<AssignmentKey, Assignment>,
    cross_file_globals: &HashMap<AssignmentKey, Assignment>,
) -> HashMap<AssignmentKey, Assignment> {
    let mut merged = cross_file_globals.clone();
    merged.extend(file_assignments.clone());
    merged
}
