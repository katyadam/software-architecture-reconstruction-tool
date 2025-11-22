use neo4rs::{BoltMap, BoltType, DeError};

use crate::contextmap::model::Dependency;

impl From<Dependency> for BoltType {
    fn from(value: Dependency) -> Self {
        let mut map = BoltMap::new();
        map.put("source_id".into(), value.source_id.into());
        map.put("target_id".into(), value.target_id.into());
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
            source_id,
            target_id,
        })
    }
}
