use models::Entity;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

mod bolt;

#[derive(ToSchema, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ContextMap {
    pub entities: Vec<Entity>,
    pub dependencies: Vec<Dependency>,
}
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq, Eq)]
pub struct Dependency {
    pub source_id: String,
    pub target_id: String,
}
