use std::collections::HashMap;

use models::{Entity, Import};

pub fn evaluate_entity_fields(imports: &[Import], entities: &mut Vec<Entity>) {
    let imports_map = get_imports_map(imports);
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
                // 2. Resolve against Imports
                else if let Some(import) = imports_map.get(inner_type) {
                    field.datatype_signature = Some(import.orig_module.clone());
                }
                // 3. Resolve against local Entities (same file classes)
                else if let Some(referenced_entity) = entities_map.get(inner_type) {
                    field.datatype_signature = Some(referenced_entity.signature.clone());
                }
            }
        }
    }
}

fn get_imports_map(imports: &[Import]) -> HashMap<String, &Import> {
    imports
        .iter()
        .map(|import| (import.codeword.clone(), import))
        .collect()
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
/// - `"List<User>"` → `"User"`
/// - `"Map<String, User>"` → `"User"` (last type argument is used — the Value in `Map<K,V>`)
/// - `"User[]"` → `"User"`
/// - `"String"` → `"String"` (identity)
pub fn extract_java_inner_type(datatype: &str) -> &str {
    let d = datatype.trim();

    // 1. Handle Arrays (e.g., "User[]" or "int[][]")
    // We strip all trailing brackets to get the base type
    if d.ends_with(']')
        && let Some(first_bracket) = d.find('[')
    {
        return extract_java_inner_type(d[..first_bracket].trim());
    }

    // 2. Handle Generics (e.g., "List<User>" or "Map<String, User>")
    if d.contains('<') && d.ends_with('>') {
        let start_index = d.find('<').unwrap();
        let end_index = d.rfind('>').unwrap();
        let inner = &d[start_index + 1..end_index].trim();

        // If it's a Map or Multi-generic, we usually want the last type (the Value)
        if inner.contains(',') {
            let parts: Vec<&str> = inner.split(',').collect();
            let last_part = parts.last().unwrap_or(inner).trim();
            return extract_java_inner_type(last_part);
        }

        return extract_java_inner_type(inner);
    }

    d
}
