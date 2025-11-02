use std::fmt::{self, Display};

use neo4rs::{BoltList, BoltMap, BoltNode, BoltString, BoltType, DeError};
use serde::{Deserialize, Serialize, de::Unexpected};
use utoipa::ToSchema;

#[derive(ToSchema, Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct Parameter {
    pub name: String,
    pub datatype: Option<String>,
    pub initial_value: Option<String>,
}

impl fmt::Display for Parameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let to_print = match (&self.datatype, &self.initial_value) {
            (None, None) => format!("{}", self.name),
            (Some(datatype), None) => format!("{}:{}", self.name, datatype),
            (None, Some(initial_value)) => format!("{}={}", self.name, initial_value),
            (Some(datatype), Some(initial_value)) => {
                format!("{}:{}={}", self.name, datatype, initial_value)
            }
        };
        write!(f, "{}", to_print)
    }
}

#[derive(ToSchema, Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum Namespace {
    Class(String),
    Module(String),
}

impl Namespace {
    pub fn get_signature(&self) -> String {
        match self {
            Namespace::Class(value) => format!("class:{}", value),
            Namespace::Module(value) => format!("module:{}", value),
        }
    }
}

#[derive(ToSchema, Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct Callable {
    pub signature: String,
    pub namespace: Namespace,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<String>,
    pub is_async: bool,
    pub is_constructor: bool,
    pub hash: String,
    pub file_path: String,
}

impl Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Namespace::Class(c) => write!(f, "class::{c}"),
            Namespace::Module(m) => write!(f, "module::{m}"),
        }
    }
}

impl Display for Callable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let params = self
            .parameters
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        write!(
            f,
            "{}{}{} {}({}) -> {} | [hash:{}]",
            if self.is_constructor {
                "Constructor: "
            } else {
                ""
            },
            if self.is_async { "async " } else { "" },
            self.namespace,
            self.signature,
            params,
            self.return_type.clone().unwrap_or_else(|| "void".into()),
            self.hash,
        )
    }
}

impl Into<BoltType> for Callable {
    fn into(self) -> BoltType {
        let mut map = BoltMap::new();
        map.put("signature".into(), self.signature.into());
        map.put("return_type".into(), self.return_type.into());
        map.put("is_async".into(), self.is_async.into());
        map.put("is_constructor".into(), self.is_constructor.into());
        map.put("hash".into(), self.hash.into());

        let namespace_json = serde_json::to_string(&self.namespace).unwrap();
        map.put("namespace".into(), namespace_json.into());

        let parameters_list: BoltType = BoltType::List(BoltList {
            value: self
                .parameters
                .into_iter()
                .map(|parameter| {
                    let json = serde_json::to_string(&parameter).unwrap();
                    BoltType::String(BoltString::new(&json))
                })
                .collect(),
        });
        map.put("parameters".into(), parameters_list);

        map.put("file_path".into(), self.file_path.into());

        BoltType::Map(map)
    }
}

impl TryFrom<BoltNode> for Callable {
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
            Ok(BoltType::String(s)) => serde_json::from_str(&s.value).unwrap(),
            _ => return Err(DeError::NoSuchProperty),
        };

        Ok(Callable {
            signature,
            namespace,
            parameters,
            return_type: Some(return_type),
            is_async,
            is_constructor,
            hash,
            file_path,
        })
    }
}
