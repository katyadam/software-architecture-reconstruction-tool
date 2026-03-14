use std::{collections::HashMap, path::PathBuf};

use models::{Entity, Import};

pub fn create_field_datatype_signature(
    file_path: &str,
    module_path: &str,
    datatype: &str,
) -> String {
    let mut path = PathBuf::from(file_path);
    path.pop();

    // Count how many `.` segments are in the module path
    let parts: Vec<&str> = module_path.split('.').collect();

    for _ in 0..parts.len() {
        path.pop();
    }
    parts.iter().for_each(|part| {
        path.push(part);
    });

    format!("{}/{}", path.display(), datatype)
}

pub fn evaluate_entity_fields(imports: &[Import], entities: &mut Vec<Entity>, file_path: &str) {
    let imports_map = get_imports_map(imports);
    let entities_map = get_entities_map(entities);

    for entity in entities {
        for field in &mut entity.fields {
            if let Some(ref raw_dt) = field.datatype {
                let inner_type = extract_inner_type(raw_dt);

                if let Some(module) = imports_map.get(inner_type) {
                    field.datatype_signature = Some(create_field_datatype_signature(
                        file_path,
                        &module.orig_module,
                        inner_type,
                    ));
                }

                if let Some(referenced_entity) = entities_map.get(inner_type) {
                    field.datatype_signature = Some(referenced_entity.signature.clone());
                }
            }
        }
    }
}

pub fn get_imports_map(imports: &[Import]) -> HashMap<String, &Import> {
    imports
        .iter()
        .map(|import| (import.codeword.clone(), import))
        .collect()
}

pub fn get_entities_map(entities: &[Entity]) -> HashMap<String, Entity> {
    entities
        .iter()
        .map(|entity| (entity.name.clone(), entity.clone()))
        .collect()
}

pub fn extract_inner_type(datatype: &str) -> &str {
    let d = datatype.trim();

    // Check if it's a generic type: Name[Inner]
    if let Some(start_index) = d.find('[')
        && let Some(end_index) = d.rfind(']') {
            let inner = &d[start_index + 1..end_index].trim();

            // Handle multiple arguments like Dict[str, User]
            // We usually care about the custom Entity, which is likely the last part
            if inner.contains(',') {
                let parts: Vec<&str> = inner.split(',').collect();
                let last_part = parts.last().unwrap_or(inner).trim();
                // Recursively unwrap in case of list[Dict[str, User]]
                return extract_inner_type(last_part);
            }

            // Recursively unwrap in case of Optional[list[User]]
            return extract_inner_type(inner);
        }
    d
}
