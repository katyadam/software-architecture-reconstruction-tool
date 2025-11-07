use models::{Callable, Namespace, Parameter};
use neo4rs::{BoltList, BoltMap, BoltNode, BoltString, BoltType, DeError};
use serde::de::Unexpected;

use crate::{
    imcg::model::{Call, ServiceCallable},
    sdg::model::Request,
};

impl Into<BoltType> for ServiceCallable {
    fn into(self) -> BoltType {
        let mut map = BoltMap::new();
        map.put("signature".into(), self.callable.signature.into());
        map.put("return_type".into(), self.callable.return_type.into());
        map.put("is_async".into(), self.callable.is_async.into());
        map.put("is_constructor".into(), self.callable.is_constructor.into());
        map.put("hash".into(), self.callable.hash.into());

        let namespace_json = serde_json::to_string(&self.callable.namespace).unwrap();
        map.put("namespace".into(), namespace_json.into());

        let parameters_list: BoltType = BoltType::List(BoltList {
            value: self
                .callable
                .parameters
                .into_iter()
                .map(|parameter| {
                    let json = serde_json::to_string(&parameter).unwrap();
                    BoltType::String(BoltString::new(&json))
                })
                .collect(),
        });
        map.put("parameters".into(), parameters_list);

        map.put("file_path".into(), self.callable.file_path.into());

        map.put("service_name".into(), self.service_name.into());

        BoltType::Map(map)
    }
}

impl TryFrom<BoltNode> for ServiceCallable {
    type Error = DeError;

    fn try_from(node: BoltNode) -> Result<Self, Self::Error> {
        let signature = match node.get("id") {
            Ok(BoltType::String(s)) => s.value,
            _ => return Err(DeError::NoSuchProperty),
        };

        let return_type = match node.get("return_type") {
            Ok(BoltType::String(s)) => s.value,
            _ => return Err(DeError::NoSuchProperty),
        };

        let is_async = match node.get("is_async") {
            Ok(BoltType::Boolean(b)) => b.value,
            _ => return Err(DeError::NoSuchProperty),
        };

        let is_constructor = match node.get("is_constructor") {
            Ok(BoltType::Boolean(b)) => b.value,
            _ => return Err(DeError::NoSuchProperty),
        };

        let hash = match node.get("hash") {
            Ok(BoltType::String(s)) => s.value,
            _ => return Err(DeError::NoSuchProperty),
        };

        let namespace: Namespace = match node.get("namespace") {
            Ok(BoltType::String(s)) => serde_json::from_str(&s.value).unwrap(),
            _ => return Err(DeError::NoSuchProperty),
        };

        let parameters = match node.get("parameters") {
            Ok(BoltType::List(l)) => l
                .value
                .into_iter()
                .map(|field| match field {
                    BoltType::String(json_field) => Ok(serde_json::from_str(
                        json_field.value.as_str(),
                    )
                    .unwrap_or(Parameter {
                        name: "Undeserialized".to_string(),
                        datatype: None,
                        initial_value: None,
                    })),
                    _ => Err(DeError::InvalidType {
                        received: Unexpected::Other("Non BoltString type").into(),
                        expected: "BoltString".to_string(),
                    }),
                })
                .collect::<Result<Vec<Parameter>, DeError>>()?,
            _ => {
                return Err(DeError::Other(
                    "Parameters argument not present in Callable".to_string(),
                ));
            }
        };

        let file_path = match node.get("file_path") {
            Ok(BoltType::String(s)) => s.value,
            _ => return Err(DeError::NoSuchProperty),
        };

        let service_name = match node.get("service_name") {
            Ok(BoltType::String(s)) => s.value,
            _ => return Err(DeError::NoSuchProperty),
        };

        Ok(ServiceCallable::new(
            Callable {
                signature,
                namespace,
                parameters,
                return_type: Some(return_type),
                is_async,
                is_constructor,
                hash,
                file_path,
            },
            service_name.into(),
        ))
    }
}

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
