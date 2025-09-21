use models::Entity;
use neo4rs::{BoltMap, BoltType, DeError};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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

impl Into<BoltType> for Dependency {
    fn into(self) -> BoltType {
        let mut map = BoltMap::new();
        map.put("source_id".into(), self.source_id.into());
        map.put("target_id".into(), self.target_id.into());
        BoltType::Map(map)
    }
}

impl TryFrom<BoltMap> for Dependency {
    type Error = DeError;

    fn try_from(node: BoltMap) -> Result<Self, Self::Error> {
        let source_id = match node.get("source") {
            Ok(BoltType::String(s)) => s.value,
            Ok(BoltType::Null(_)) => {
                return Err(DeError::Other("Source is NULL!".to_string()));
            }
            _ => return Err(DeError::NoSuchProperty),
        };

        let target_id = match node.get("target") {
            Ok(BoltType::String(s)) => s.value,
            Ok(BoltType::Null(_)) => {
                return Err(DeError::Other("Target is NULL!".to_string()));
            }
            _ => return Err(DeError::NoSuchProperty),
        };

        Ok(Dependency {
            source_id: source_id,
            target_id: target_id,
        })
    }
}
