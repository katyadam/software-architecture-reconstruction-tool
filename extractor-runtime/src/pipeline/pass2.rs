use std::collections::HashMap;

use java_extractor::extraction::{
    calls::evaluator::evaluate_invocations as java_evaluate_invocations,
    entities::evaluator::evaluate_entity_fields as java_evaluate_entity_fields,
    restcalls::identification::{
        spring::SpringIdentificationStrategy, strategy::IdentificationStrategy,
    },
};
use models::{
    Assignment, AssignmentKey, Scope,
    ir::{
        language::Language,
        project::{ConstantValue, ImportGraph, ProjectIR, TypedFileRecord},
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
    re_identify_restcalls(&mut files);
    let class_hierarchy = build_class_hierarchy(&files, &import_graph);

    let constants = collect_constants(&files);

    ProjectIR {
        files,
        import_graph,
        class_hierarchy,
        constants,
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

/// Re-run REST call identification on type-resolved call statements.
///
/// Java's Spring identification requires `call.invoked_on == "RestTemplate"`, which is only
/// populated by `evaluate_invocations` (Pass 2). Pass 1 runs identification before types are
/// resolved, so `raw_restcalls` for Spring clients is empty after extraction.
fn re_identify_restcalls(files: &mut [TypedFileRecord]) {
    let strategy = SpringIdentificationStrategy::new();
    for file in files.iter_mut() {
        if file.language != Language::Java {
            continue;
        }
        let identified: Vec<_> = file
            .call_statements
            .iter()
            .filter_map(|call| strategy.identify_restcall(call, &file.file_path))
            .collect();
        file.raw_restcalls.extend(identified);
    }
}

/// Collect project-wide constants from global-scope assignments with literal values.
///
/// A constant is any `Scope::Global` assignment whose name is `UPPER_SNAKE_CASE`
/// and whose value is a string literal, numeric literal, or boolean.
/// First definition wins on name collisions (import order is non-deterministic).
fn collect_constants(files: &[TypedFileRecord]) -> HashMap<String, ConstantValue> {
    let mut constants = HashMap::new();
    for file in files {
        for (key, assignment) in &file.assignments {
            if matches!(key.scope, Scope::Global) && is_constant_name(&key.variable_name) {
                constants
                    .entry(key.variable_name.clone())
                    .or_insert_with(|| ConstantValue {
                        name: key.variable_name.clone(),
                        value: assignment.value.clone(),
                        source_file: file.file_path.clone(),
                    });
            }
        }
    }
    constants
}

/// A constant name is non-empty and consists only of uppercase ASCII letters, digits, and `_`.
fn is_constant_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
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
