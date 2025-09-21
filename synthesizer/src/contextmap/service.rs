use std::collections::HashMap;

use models::Entity;

use crate::contextmap::model::{ContextMap, Dependency};

pub fn build_context_map(entities: &Vec<Entity>) -> ContextMap {
    let collected_dependencies = connect_entities(entities);
    ContextMap {
        entities: entities.to_vec(),
        dependencies: collected_dependencies,
    }
}

pub fn connect_entities(entities: &Vec<Entity>) -> Vec<Dependency> {
    let entities_map = create_entities_map(entities);

    entities
        .iter()
        .flat_map(|entity| {
            entity.fields.iter().filter_map(|field| {
                field
                    .datatype_signature
                    .as_ref()
                    .filter(|sig| entities_map.contains_key(*sig))
                    .map(|sig| Dependency {
                        source_id: entity.signature.clone(),
                        target_id: sig.clone(),
                    })
            })
        })
        .collect()
}

pub fn create_entities_map(entities: &Vec<Entity>) -> HashMap<String, &Entity> {
    entities
        .into_iter()
        .map(|entity| (entity.signature.clone(), entity))
        .collect()
}
