use std::collections::HashMap;

use java_extractor::extraction::entities::evaluator::evaluate_entity_fields as java_evaluate_entity_fields;
use models::ir::{
    language::Language,
    project::{ClassHierarchy, ImportGraph, ProjectIR, TypedFileRecord},
    syntax::FileRecord,
};
use python_extractor::extraction::entities::evaluator::evaluate_entity_fields as python_evaluate_entity_fields;
use statix::import_graph::build_import_graph;

/// Pass 2: Produce a `ProjectIR` from all `FileRecord`s collected in Pass 1.
///
/// Currently implements:
///   - Import graph construction (cross-file symbol resolution)
///   - Entity field type resolution (`Field.datatype_signature` population)
///
/// Remaining steps (class hierarchy, constant collection) are added in
/// subsequent Phase C increments.
pub fn build_project_ir(file_records: Vec<FileRecord>) -> ProjectIR {
    let import_graph = build_import_graph(&file_records);

    let mut files: Vec<TypedFileRecord> = file_records
        .into_iter()
        .map(TypedFileRecord::from)
        .collect();

    resolve_entity_fields(&mut files, &import_graph);

    ProjectIR {
        files,
        import_graph,
        class_hierarchy: ClassHierarchy {
            parents: HashMap::new(),
            children: HashMap::new(),
        },
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
