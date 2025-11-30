use models::Entity;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

mod bolt;

#[derive(ToSchema, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ContextMap {
    pub entities: Vec<AssignedEntity>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq, Eq)]
pub struct AssignedEntity {
    pub entity: Entity,
    pub service_name: String,
}

impl AssignedEntity {
    pub fn new(entity: Entity, service_name: String) -> Self {
        Self {
            entity,
            service_name,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq, Eq)]
pub struct Dependency {
    pub source_id: String,
    pub target_id: String,
}
