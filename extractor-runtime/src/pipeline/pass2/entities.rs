use models::ir::{
    language::Language,
    project::{ImportGraph, TypedFileRecord},
};

use python_extractor::extraction::entities::evaluator::evaluate_entity_fields as python_evaluate_entity_fields;

use java_extractor::extraction::entities::evaluator::evaluate_entity_fields as java_evaluate_entity_fields;

/// Dispatch entity field resolution to the language-specific evaluator for each file.
pub fn resolve_entity_fields(files: &mut [TypedFileRecord], import_graph: &ImportGraph) {
    for file in files.iter_mut() {
        match file.language {
            Language::Java => {
                java_evaluate_entity_fields(&mut file.entities, &file.file_path, import_graph)
            }
            Language::Python => {
                python_evaluate_entity_fields(&mut file.entities, &file.file_path, import_graph)
            }
            Language::Go => {}
        }
    }
}
