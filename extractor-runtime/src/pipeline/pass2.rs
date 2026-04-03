use std::collections::HashMap;

use models::ir::{
    project::{ClassHierarchy, ProjectIR, TypedFileRecord},
    syntax::FileRecord,
};
use statix::import_graph::build_import_graph;

/// Pass 2: Produce a `ProjectIR` from all `FileRecord`s collected in Pass 1.
///
/// Currently implements:
///   - Import graph construction (cross-file symbol resolution)
///
/// Remaining steps (entity type resolution, class hierarchy, constant
/// collection) are added in subsequent Phase C increments.
pub fn build_project_ir(file_records: Vec<FileRecord>) -> ProjectIR {
    let import_graph = build_import_graph(&file_records);

    let files = file_records
        .into_iter()
        .map(TypedFileRecord::from)
        .collect();

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
