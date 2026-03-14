use std::collections::HashMap;

use log::error;
use models::Entity;
use neo4rs::{BoltMap, BoltNode, BoltType, DeError};

use crate::contextmap::model::{AssignedEntity, Dependency};

impl From<AssignedEntity> for BoltType {
    fn from(value: AssignedEntity) -> Self {
        let mut map = match BoltType::from(value.entity) {
            BoltType::Map(m) => m,
            _ => {
                error!("Expected BoltType::Map in From<AssignedEntity> for BoltType");
                BoltMap {
                    value: HashMap::new(),
                }
            }
        };

        map.put("service_name".into(), value.service_name.into());

        BoltType::Map(map)
    }
}

impl TryFrom<BoltNode> for AssignedEntity {
    type Error = DeError; // or a custom error type

    fn try_from(node: BoltNode) -> Result<Self, Self::Error> {
        let service_name = match node.get("service_name") {
            Ok(BoltType::String(s)) => s.value,
            Ok(BoltType::Null(_)) => {
                return Err(DeError::Other("Service Name is NULL!".to_string()));
            }
            _ => return Err(DeError::NoSuchProperty),
        };

        let entity = Entity::try_from(node)?;

        Ok(AssignedEntity {
            entity,
            service_name,
        })
    }
}

impl From<Dependency> for BoltType {
    fn from(value: Dependency) -> Self {
        let mut map = BoltMap::new();
        map.put("source_id".into(), value.source_id.into());
        map.put("target_id".into(), value.target_id.into());
        map.put("source_multiplier".into(), value.source_multiplier.into());
        map.put("target_multiplier".into(), value.target_multiplier.into());
        BoltType::Map(map)
    }
}

impl TryFrom<BoltNode> for Dependency {
    type Error = DeError;

    fn try_from(node: BoltNode) -> Result<Self, Self::Error> {
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

        let source_multiplier = match node.get("source_multiplier") {
            Ok(BoltType::String(s)) => s.value,
            Ok(BoltType::Null(_)) => {
                return Err(DeError::Other("Source Multiplier is NULL!".to_string()));
            }
            _ => return Err(DeError::NoSuchProperty),
        };

        let target_multiplier = match node.get("target_multiplier") {
            Ok(BoltType::String(s)) => s.value,
            Ok(BoltType::Null(_)) => {
                return Err(DeError::Other("Target Multiplier is NULL!".to_string()));
            }
            _ => return Err(DeError::NoSuchProperty),
        };

        Ok(Dependency {
            source_id,
            target_id,
            source_multiplier,
            target_multiplier,
        })
    }
}
