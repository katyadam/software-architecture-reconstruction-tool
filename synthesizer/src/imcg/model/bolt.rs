use neo4rs::{BoltMap, BoltType, DeError};

use crate::{imcg::model::Call, sdg::model::Request};

impl Into<BoltType> for Call {
    fn into(self) -> BoltType {
        let mut map = BoltMap::new();
        map.put("source_id".into(), self.source_id.into());
        map.put("target_id".into(), self.target_id.into());

        let request_json = serde_json::to_string(&self.request).unwrap();
        map.put("request".into(), request_json.into());

        BoltType::Map(map)
    }
}

impl TryFrom<BoltMap> for Call {
    type Error = DeError;

    fn try_from(node: BoltMap) -> Result<Self, Self::Error> {
        let source_id = match node.get("source_id") {
            Ok(BoltType::String(s)) => s.value,
            Ok(BoltType::Null(_)) => {
                return Err(DeError::Other("Source is NULL!".to_string()));
            }
            _ => return Err(DeError::NoSuchProperty),
        };

        let target_id = match node.get("target_id") {
            Ok(BoltType::String(s)) => s.value,
            Ok(BoltType::Null(_)) => {
                return Err(DeError::Other("Target is NULL!".to_string()));
            }
            _ => return Err(DeError::NoSuchProperty),
        };

        let request: Request = match node.get("request") {
            Ok(BoltType::String(s)) => serde_json::from_str(&s.value).unwrap(),
            _ => return Err(DeError::NoSuchProperty),
        };

        Ok(Call {
            source_id,
            target_id,
            request: Some(request),
        })
    }
}
