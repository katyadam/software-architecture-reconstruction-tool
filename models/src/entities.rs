use neo4rs::{BoltList, BoltMap, BoltNode, BoltString, BoltType, DeError};
use serde::{Deserialize, Serialize, de::Unexpected};
use utoipa::ToSchema;

#[derive(Debug, PartialEq, Eq, ToSchema, Serialize, Deserialize, Clone)]
pub struct Entity {
    pub name: String,
    pub superclasses: Vec<String>,
    pub fields: Vec<Field>,
    pub signature: String,
    pub file_path: String,
}

#[derive(Debug, PartialEq, Eq, ToSchema, Serialize, Deserialize, Clone)]
pub struct Field {
    pub name: String,
    pub datatype: Option<String>,
    pub initial_value: Option<String>,
    pub datatype_signature: Option<String>,
}

impl From<Entity> for BoltType {
    fn from(value: Entity) -> Self {
        let mut map = BoltMap::new();
        map.put("name".into(), value.name.into());
        map.put("signature".into(), value.signature.into());
        map.put("file_path".into(), value.file_path.into());

        let superclasses_list: BoltType = BoltType::List(BoltList {
            value: value
                .superclasses
                .into_iter()
                .map(|superclass| superclass.into())
                .collect(),
        });

        map.put("superclasses".into(), superclasses_list);

        let fields_list: BoltType = BoltType::List(BoltList {
            value: value
                .fields
                .into_iter()
                .map(|field| {
                    let json = serde_json::to_string(&field).unwrap();
                    BoltType::String(BoltString::new(json.as_str()))
                })
                .collect(),
        });
        map.put("fields".into(), fields_list);

        BoltType::Map(map)
    }
}

impl TryFrom<BoltNode> for Entity {
    type Error = DeError;

    fn try_from(node: BoltNode) -> Result<Self, Self::Error> {
        let signature = match node.get("id") {
            Ok(BoltType::String(s)) => s.value,
            _ => return Err(DeError::NoSuchProperty),
        };

        let name = match node.get("name") {
            Ok(BoltType::String(s)) => s.value,
            _ => return Err(DeError::NoSuchProperty),
        };

        let superclasses = match node.get("superclasses") {
            Ok(BoltType::List(l)) => l
                .value
                .into_iter()
                .map(|sc| match sc {
                    BoltType::String(sc) => Ok(sc.value),
                    _ => Err(DeError::InvalidType {
                        received: Unexpected::Other("Non BoltString type").into(),
                        expected: "BoltString".to_string(),
                    }),
                })
                .collect::<Result<Vec<String>, DeError>>()?,
            _ => return Err(DeError::NoSuchProperty),
        };

        let fields = match node.get("fields") {
            Ok(BoltType::List(l)) => l
                .value
                .into_iter()
                .map(|field| match field {
                    BoltType::String(json_field) => Ok(serde_json::from_str(
                        json_field.value.as_str(),
                    )
                    .unwrap_or(Field {
                        name: "Undeserialized".to_string(),
                        datatype: None,
                        initial_value: None,
                        datatype_signature: None,
                    })),
                    _ => Err(DeError::InvalidType {
                        received: Unexpected::Other("Non BoltString type").into(),
                        expected: "BoltString".to_string(),
                    }),
                })
                .collect::<Result<Vec<Field>, DeError>>()?,
            _ => {
                return Err(DeError::Other(
                    "Fields argument not present in Entity".to_string(),
                ));
            }
        };

        let file_path = match node.get("file_path") {
            Ok(BoltType::String(s)) => s.value,
            _ => return Err(DeError::NoSuchProperty),
        };
        Ok(Entity {
            signature,
            name,
            superclasses,
            fields,
            file_path,
        })
    }
}
