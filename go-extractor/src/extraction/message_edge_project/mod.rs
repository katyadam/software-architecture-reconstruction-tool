use std::collections::HashSet;

use models::{CallStatement, MessageRole, ParsedCallable, ir::project::TypedFileRecord};

mod resolution;
mod values;

use super::shared::package_path;

/// Replaces Go message edges with concrete variants derived from project call sites.
pub(super) fn resolve_message_edges(files: &mut [TypedFileRecord]) {
    let snapshots = files
        .iter()
        .filter(|file| file.language == models::ir::language::Language::Go)
        .map(FileSnapshot::from)
        .collect::<Vec<_>>();
    for file in files
        .iter_mut()
        .filter(|file| file.language == models::ir::language::Language::Go)
    {
        file.raw_message_edges = file
            .raw_message_edges
            .iter()
            .flat_map(|edge| {
                let Some(source) = snapshot_for(&snapshots, &file.file_path) else {
                    return vec![edge.clone()];
                };
                let Some(callable) = source
                    .callables
                    .iter()
                    .find(|callable| callable.metadata.hash == edge.function_hash)
                else {
                    return vec![edge.clone()];
                };
                let known_values = values::known_values_for_source(&snapshots, &source.file_path);
                let declared = resolution::resolve_declared_edge(
                    edge,
                    callable,
                    source,
                    &snapshots,
                    &known_values,
                );
                if (edge.role != MessageRole::Producer && callable.metadata.name.starts_with("New"))
                    || has_ambiguous_method_name(callable, &snapshots)
                {
                    return declared;
                }
                let mut resolved = Vec::new();
                for invocation in matching_calls(callable, source, &snapshots) {
                    for edge in &declared {
                        resolved.extend(resolution::resolve_invocation(
                            edge,
                            callable,
                            invocation,
                            &snapshots,
                            &known_values,
                            0,
                            &mut HashSet::new(),
                        ));
                    }
                }
                if resolved.is_empty() {
                    declared
                } else {
                    resolved
                }
            })
            .collect();
    }
}

/// Returns true when multiple method implementations share a name across the project.
fn has_ambiguous_method_name(callable: &ParsedCallable, files: &[FileSnapshot]) -> bool {
    matches!(callable.metadata.namespace, models::Namespace::Class(_))
        && files
            .iter()
            .flat_map(|file| &file.callables)
            .filter(|candidate| candidate.metadata.name == callable.metadata.name)
            .count()
            > 1
}

pub(super) struct FileSnapshot {
    pub(super) file_path: String,
    pub(super) import_modules: Vec<String>,
    pub(super) callables: Vec<ParsedCallable>,
    pub(super) calls: Vec<CallStatement>,
    pub(super) assignments: Vec<(String, String)>,
}

#[derive(Clone, Copy)]
pub(super) struct Invocation<'a> {
    pub(super) call: &'a CallStatement,
    pub(super) file: &'a FileSnapshot,
}

impl From<&TypedFileRecord> for FileSnapshot {
    /// Retains the call, callable, and assignment metadata needed for project resolution.
    fn from(file: &TypedFileRecord) -> Self {
        Self {
            file_path: file.file_path.clone(),
            import_modules: file
                .imports
                .iter()
                .map(|import| import.orig_module.clone())
                .collect(),
            callables: file.callables.clone(),
            calls: file.call_statements.clone(),
            assignments: file
                .assignments
                .values()
                .map(|assignment| (assignment.variable_name.clone(), assignment.value.clone()))
                .collect(),
        }
    }
}

/// Finds the snapshot belonging to a known project file.
pub(super) fn snapshot_for<'a>(
    files: &'a [FileSnapshot],
    file_path: &str,
) -> Option<&'a FileSnapshot> {
    files.iter().find(|file| file.file_path == file_path)
}

/// Finds calls to a callable from its own package or an importing Go package.
pub(super) fn matching_calls<'a>(
    callable: &'a ParsedCallable,
    target_file: &FileSnapshot,
    files: &'a [FileSnapshot],
) -> Vec<Invocation<'a>> {
    let calls = files
        .iter()
        .filter(|file| package_matches(file, target_file))
        .flat_map(|file| file.calls.iter().map(move |call| Invocation { call, file }))
        .filter(|invocation| {
            invocation.call.function_name.rsplit('.').next()
                == Some(callable.metadata.name.as_str())
                && invocation.call.arguments.len() == callable.metadata.parameters.len()
        })
        .collect::<Vec<_>>();
    if !calls.is_empty() || !matches!(callable.metadata.namespace, models::Namespace::Class(_)) {
        return calls;
    }
    let matching_definitions = files
        .iter()
        .flat_map(|file| &file.callables)
        .filter(|candidate| candidate.metadata.name == callable.metadata.name)
        .count();
    if matching_definitions != 1 {
        return Vec::new();
    }

    files
        .iter()
        .flat_map(|file| file.calls.iter().map(move |call| Invocation { call, file }))
        .filter(|invocation| {
            invocation.call.function_name.rsplit('.').next()
                == Some(callable.metadata.name.as_str())
                && invocation.call.arguments.len() == callable.metadata.parameters.len()
        })
        .collect()
}

/// Checks whether a caller belongs to or imports the callable's package.
fn package_matches(caller: &FileSnapshot, target_file: &FileSnapshot) -> bool {
    let package = package_path(&target_file.file_path);
    if package_path(&caller.file_path) == package {
        return true;
    }
    let suffix = package
        .rsplit('/')
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("/");
    caller
        .import_modules
        .iter()
        .any(|module| module.replace('\\', "/").ends_with(&suffix))
}
