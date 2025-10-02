use std::collections::HashMap;

use crate::{
    contextmap::model::{ContextMap, Dependency},
    errors::builder::BuilderError,
};

use models::Entity;
pub trait ContextMapBuilder {
    fn build(&self, entities: &Vec<Entity>) -> Result<ContextMap, BuilderError>;
}

pub struct ContextMapBuilderImpl {}

impl ContextMapBuilder for ContextMapBuilderImpl {
    fn build(&self, entities: &Vec<Entity>) -> Result<ContextMap, BuilderError> {
        let collected_dependencies = Self::connect_entities(entities);
        Ok(ContextMap {
            entities: entities.to_vec(),
            dependencies: collected_dependencies,
        })
    }
}

impl ContextMapBuilderImpl {
    pub fn new() -> Self {
        Self {}
    }

    fn connect_entities(entities: &Vec<Entity>) -> Vec<Dependency> {
        let entities_map = Self::create_entities_map(entities);

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

    fn create_entities_map(entities: &Vec<Entity>) -> HashMap<String, &Entity> {
        entities
            .into_iter()
            .map(|entity| (entity.signature.clone(), entity))
            .collect()
    }
}
