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
            if let Some(ref raw_datatype) = field.datatype {
                // Extract the "Real" type: e.g., "List<User>" -> "User"
                let inner_type = extract_java_inner_type(raw_datatype);

                // 1. Check if the inner type is already an FQDN
                if is_fqdn(inner_type) {
                    field.datatype_signature = Some(inner_type.to_string());
                }
                // 2. Resolve against ImportGraph (cross-file)
                else if let Some(ri) = import_graph.lookup(file_path, inner_type) {
                    if matches!(ri.kind, ImportKind::Entity) {
                        field.datatype_signature = Some(ri.fully_qualified_name.clone());
                    }
                }
                // 3. Resolve against local Entities (same file classes)
                else if let Some(referenced_entity) = entities_map.get(inner_type) {
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

fn is_fqdn(datatype: &str) -> bool {
    datatype.contains(".")
}

/// Recursively unwraps arrays and generic wrappers to find the element type.
///
/// Examples:
/// - `"List<User>"` -> `"User"`
/// - `"Map<String, User>"` -> `"User"` (last type argument is used — the Value in `Map<K,V>`)
/// - `"User[]"` -> `"User"`
/// - `"String"` -> `"String"` (identity)
pub fn extract_java_inner_type(datatype: &str) -> &str {
    let d = datatype.trim();

    // Handle Arrays (e.g., "User[]" or "int[][]") — strip trailing brackets first
    if d.ends_with(']')
        && let Some(first_bracket) = d.find('[')
    {
        return extract_java_inner_type(d[..first_bracket].trim());
    }

    // Delegate generic unwrapping to the shared helper
    statix::strings::extract_inner_generic_type(d, '<', '>')
}
