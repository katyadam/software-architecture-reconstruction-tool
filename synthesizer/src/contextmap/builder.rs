use std::collections::HashMap;

use crate::{
    contextmap::model::{AssignedEntity, ContextMap, Dependency},
    errors::builder::BuilderError,
    utils::assign_service_description_to_file,
};

use models::{Entity, configuration::ServiceDescription};
pub trait ContextMapBuilder {
    fn build(
        &self,
        entities: &[Entity],
        service_descs: &[ServiceDescription],
    ) -> Result<ContextMap, BuilderError>;
}

pub struct ContextMapBuilderImpl {}

impl ContextMapBuilder for ContextMapBuilderImpl {
    fn build(
        &self,
        entities: &[Entity],
        service_descs: &[ServiceDescription],
    ) -> Result<ContextMap, BuilderError> {
        let collected_dependencies = Self::connect_entities(entities);
        let assigned_entities = self.get_assigned_entities(entities, service_descs);
        Ok(ContextMap {
            entities: assigned_entities,
            dependencies: collected_dependencies,
        })
    }
}

impl Default for ContextMapBuilderImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextMapBuilderImpl {
    pub fn new() -> Self {
        Self {}
    }

    fn get_assigned_entities(
        &self,
        entities: &[Entity],
        service_descs: &[ServiceDescription],
    ) -> Vec<AssignedEntity> {
        entities
            .iter()
            .map(|entity| {
                let service_desc =
                    assign_service_description_to_file(&entity.file_path, service_descs);
                AssignedEntity::new(entity.clone(), service_desc.name)
            })
            .collect()
    }

    fn connect_entities(entities: &[Entity]) -> Vec<Dependency> {
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

    fn create_entities_map(entities: &[Entity]) -> HashMap<String, &Entity> {
        entities
            .iter()
            .map(|entity| (entity.signature.clone(), entity))
            .collect()
    }
}
