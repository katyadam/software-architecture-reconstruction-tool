use std::collections::HashMap;

use models::{
    Entity,
    ir::project::{ImportGraph, ImportKind},
};

pub fn evaluate_entity_fields(
    entities: &mut Vec<Entity>,
    file_path: &str,
    import_graph: &ImportGraph,
) {
    let entities_map = get_entities_map(entities);

    for entity in entities {
        for field in &mut entity.fields {
            if let Some(ref raw_dt) = field.datatype {
                let inner_type = extract_inner_type(raw_dt);

                // 1. Resolve via ImportGraph (cross-file)
                if let Some(ri) = import_graph.lookup(file_path, inner_type)
                    && matches!(ri.kind, ImportKind::Entity)
                {
                    field.datatype_signature = Some(ri.fully_qualified_name.clone());
                    continue;
                }

                // 2. Resolve against local Entities (same file classes)
                if let Some(referenced_entity) = entities_map.get(inner_type) {
                    field.datatype_signature = Some(referenced_entity.signature.clone());
                }
            }
        }
    }
}

fn get_entities_map(entities: &[Entity]) -> HashMap<String, Entity> {
    entities
        .iter()
        .map(|entity| (entity.name.clone(), entity.clone()))
        .collect()
}

/// Recursively unwraps generic brackets to find the element type.
///
/// Examples:
/// - `"list[User]"` -> `"User"`
/// - `"Dict[str, User]"` -> `"User"` (last argument; the Value in `Dict[K, V]`)
/// - `"Optional[list[User]]"` -> `"User"` (unwrapped recursively)
/// - `"User"` -> `"User"` (identity)
pub fn extract_inner_type(datatype: &str) -> &str {
    statix::strings::extract_inner_generic_type(datatype, '[', ']')
}
