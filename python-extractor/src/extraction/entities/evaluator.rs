use std::{collections::HashMap, path::PathBuf};

use models::{Entity, Import};

/// Builds a file-system-style fully-qualified path for a field's type.
///
/// Starting from `file_path`, navigates up by the number of segments in `module_path`
/// (treating each `.`-separated segment as one directory level), then descends
/// through those segments and appends `datatype`.
///
/// Example: `file_path = "a/b/c.py"`, `module_path = "x.y"`, `datatype = "User"`
/// → `"a/x/y/User"`
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

/// Recursively unwraps generic brackets to find the element type.
///
/// Examples:
/// - `"list[User]"` → `"User"`
/// - `"Dict[str, User]"` → `"User"` (last argument; the Value in `Dict[K, V]`)
/// - `"Optional[list[User]]"` → `"User"` (unwrapped recursively)
/// - `"User"` → `"User"` (identity)
pub fn extract_inner_type(datatype: &str) -> &str {
    statix::strings::extract_inner_generic_type(datatype, '[', ']')
}
