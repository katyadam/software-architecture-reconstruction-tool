use std::collections::HashMap;

use models::{Entity, Import};

pub fn evaluate_entity_fields(imports: &[Import], entities: &mut Vec<Entity>) {
    let imports_map = get_imports_map(&imports);
    let entities_map = get_entities_map(entities);

    for entity in entities {
        for field in &mut entity.fields {
            if let Some(ref datatype) = field.datatype {
                // Datatype already has full path to its entity (class)
                if is_fqdn(datatype) {
                    field.datatype_signature = Some(datatype.clone());
                }
                // Matches if used datatype has been imported
                if imports_map.contains_key(datatype) {
                    let import = imports_map.get(datatype).unwrap();
                    field.datatype_signature = Some(import.orig_module.clone());
                }
                // Matches if in single file there are multiple entities (classes)
                if entities_map.contains_key(datatype) {
                    let referenced_entity = entities_map.get(datatype).unwrap();
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
