use std::{collections::HashMap, path::PathBuf};

use models::{Entity, Import};

pub fn create_field_datatype_signature(
    file_path: &str,
    module_path: &String,
    datatype: &String,
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

pub fn evaluate_entity_fields(imports: &Vec<Import>, entities: &mut Vec<Entity>, file_path: &str) {
    let imports_map = get_imports_map(imports);
    let entities_map = get_entities_map(&entities);
    for entity in entities {
        for field in &mut entity.fields {
            if let Some(ref datatype) = field.datatype {
                if imports_map.contains_key(datatype) {
                    let module = imports_map.get(datatype).unwrap();
                    field.datatype_signature = Some(create_field_datatype_signature(
                        file_path,
                        &module.orig_module,
                        datatype,
                    ));
                }
                if entities_map.contains_key(datatype) {
                    let referenced_entity = entities_map.get(datatype).unwrap();
                    field.datatype_signature = Some(referenced_entity.signature.clone());
                }
            }
        }
    }
}

pub fn get_imports_map(imports: &Vec<Import>) -> HashMap<String, &Import> {
    imports
        .into_iter()
        .map(|import| (import.codeword.clone(), import))
        .collect()
}

pub fn get_entities_map(entities: &Vec<Entity>) -> HashMap<String, Entity> {
    entities
        .into_iter()
        .map(|entity| (entity.name.clone(), entity.clone()))
        .collect()
}
