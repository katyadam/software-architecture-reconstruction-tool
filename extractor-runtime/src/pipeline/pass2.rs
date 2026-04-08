use std::collections::HashMap;

use java_extractor::extraction::{
    calls::evaluator::evaluate_invocations as java_evaluate_invocations,
    entities::evaluator::evaluate_entity_fields as java_evaluate_entity_fields,
};
use models::{
    Assignment, AssignmentKey, Scope,
    ir::{
        language::Language,
        project::{ImportGraph, ProjectIR, TypedFileRecord},
        syntax::FileRecord,
    },
};
use python_extractor::extraction::{
    calls::evaluator::evaluate_invocations_on_statements as python_evaluate_invocations_on_statements,
    entities::evaluator::evaluate_entity_fields as python_evaluate_entity_fields,
};
use statix::{class_hierarchy::build_class_hierarchy, import_graph::build_import_graph};

/// Pass 2: Produce a `ProjectIR` from all `FileRecord`s collected in Pass 1.
///
/// Implements:
///   - Import graph construction (cross-file symbol resolution)
///   - Entity field type resolution (`Field.datatype_signature` population)
///   - Call type inference (`CallStatement.invoked_on` and `Argument.datatype` population)
///   - Class hierarchy building (`ClassHierarchy.parents` and `.children` population)
pub fn build_project_ir(file_records: Vec<FileRecord>) -> ProjectIR {
    let import_graph = build_import_graph(&file_records);

    let mut files: Vec<TypedFileRecord> = file_records
        .into_iter()
        .map(TypedFileRecord::from)
        .collect();

    resolve_entity_fields(&mut files, &import_graph);
    resolve_call_argument_types(&mut files);
    let class_hierarchy = build_class_hierarchy(&files, &import_graph);

    ProjectIR {
        files,
        import_graph,
        class_hierarchy,
        constants: HashMap::new(),
    }
}

/// Dispatch entity field resolution to the language-specific evaluator for each file.
fn resolve_entity_fields(files: &mut [TypedFileRecord], import_graph: &ImportGraph) {
    for file in files.iter_mut() {
        match file.language {
            Language::Java => {
                java_evaluate_entity_fields(&mut file.entities, &file.file_path, import_graph)
            }
            Language::Python => {
                python_evaluate_entity_fields(&mut file.entities, &file.file_path, import_graph)
            }
        }
    }
}

/// Dispatch call type inference to the language-specific evaluator for each file,
/// using a merged map of file-local assignments supplemented by cross-file globals.
fn resolve_call_argument_types(files: &mut [TypedFileRecord]) {
    let cross_file_globals = build_cross_file_globals(files);

    for file in files.iter_mut() {
        let assignments = merged_assignments(&file.assignments, &cross_file_globals);
        match file.language {
            Language::Java => java_evaluate_invocations(&mut file.call_statements, &assignments),
            Language::Python => {
                python_evaluate_invocations_on_statements(&mut file.call_statements, &assignments)
            }
        }
    }
}

/// Build a map of all global-scope assignments across all files.
/// Used as a cross-file fallback when per-file assignment maps lack a variable.
fn build_cross_file_globals(files: &[TypedFileRecord]) -> HashMap<AssignmentKey, Assignment> {
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
fn merged_assignments(
    file_assignments: &HashMap<AssignmentKey, Assignment>,
    cross_file_globals: &HashMap<AssignmentKey, Assignment>,
) -> HashMap<AssignmentKey, Assignment> {
    let mut merged = cross_file_globals.clone();
    merged.extend(file_assignments.clone());
    merged
}
