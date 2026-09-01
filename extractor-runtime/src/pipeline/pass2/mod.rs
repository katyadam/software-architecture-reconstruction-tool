use models::ir::{
    project::{ProjectIR, TypedFileRecord},
    syntax::FileRecord,
};
use statix::{class_hierarchy::build_class_hierarchy, import_graph::build_import_graph};

use crate::pipeline::pass2::{
    callables::build_project_global_callables, constants::collect_constants,
    entities::resolve_entity_fields, identification::identify_edges,
    type_inference::resolve_call_argument_types,
};

mod assignments;
pub mod callables;
mod constants;
mod endpoints;
mod entities;
mod identification;
mod type_inference;

/// Pass 2: Produce a `ProjectIR` from all `FileRecord`s collected in Pass 1.
///
/// Implements:
///   - Import graph construction (cross-file symbol resolution)
///   - Entity field type resolution (`Field.datatype_signature` population)
///   - Call type inference (`CallStatement.invoked_on` and `Argument.datatype` population)
///   - REST call and message edge identification (per-language, see `identification`)
///   - Class hierarchy building (`ClassHierarchy.parents` and `.children` population)
pub fn build_project_ir(file_records: Vec<FileRecord>) -> ProjectIR {
    let import_graph = build_import_graph(&file_records);

    let mut files: Vec<TypedFileRecord> = file_records
        .into_iter()
        .map(TypedFileRecord::from)
        .collect();

    resolve_entity_fields(&mut files, &import_graph);
    resolve_call_argument_types(&mut files);
    identify_edges(&mut files);
    endpoints::post_process_endpoints(&mut files);
    let class_hierarchy = build_class_hierarchy(&files, &import_graph);

    let constants = collect_constants(&files);
    let callable_map = build_project_global_callables(&files);
    let proto_services = files
        .iter()
        .flat_map(|file| file.proto_services.clone())
        .collect();

    ProjectIR {
        files,
        import_graph,
        class_hierarchy,
        constants,
        callable_map,
        proto_services,
    }
}
