use models::{Endpoint, RestCall};
use neo4rs::{BoltList, BoltMap, BoltNode, BoltString, BoltType, DeError};
use serde::{Deserialize, Serialize, de::Unexpected};
use utoipa::ToSchema;

#[derive(ToSchema, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SDG {
    pub services: Vec<Service>,
    pub connections: Vec<Connection>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, PartialEq, Eq)]
pub struct Service {
    pub name: String,
    pub endpoints: Vec<Endpoint>,
}

impl Into<BoltType> for Service {
    fn into(self) -> BoltType {
        let mut map = BoltMap::new();

        map.put("name".into(), self.name.into());

        let serialized_endpoints: BoltType = BoltType::List(BoltList {
            value: self
                .endpoints
                .into_iter()
                .map(|endpoint| {
                    let endpoint_json = serde_json::to_string(&endpoint).unwrap();
                    BoltType::String(BoltString::new(&endpoint_json))
                })
                .collect(),
        });
        map.put("endpoints".into(), serialized_endpoints);

        BoltType::Map(map)
    }
}

impl TryFrom<BoltNode> for Service {
    type Error = DeError;

    fn try_from(node: BoltNode) -> Result<Self, Self::Error> {
        let name = match node.get("name") {
            Ok(BoltType::String(s)) => s.value,
            Ok(BoltType::Null(_)) => {
                return Err(DeError::Other("Source is NULL!".to_string()));
            }
            _ => return Err(DeError::NoSuchProperty),
        };

        let endpoints = match node.get("endpoints") {
            Ok(BoltType::List(l)) => l
                .value
                .into_iter()
                .map(|endpoint| match endpoint {
                    BoltType::String(json_field) => Ok(serde_json::from_str(
                        json_field.value.as_str(),
                    )
                    .unwrap_or(Endpoint {
                        uri: "Bad Deserialization!".to_string(),
                        file_path: "Bad Deserialization!".to_string(),
                        function_name: "Bad Deserialization!".to_string(),
                        http_method: models::HttpMethod::DELETE,
                        parameters: vec![],
                    })),
                    _ => Err(DeError::InvalidType {
                        received: Unexpected::Other("Non BoltString type").into(),
                        expected: "BoltString".to_string(),
                    }),
                })
                .collect::<Result<Vec<Endpoint>, DeError>>()?,
            _ => {
                return Err(DeError::Other(
                    "Endpoints argument not present in Service".to_string(),
                ));
            }
        };

        Ok(Service { name, endpoints })
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, PartialEq, Eq)]
pub struct Connection {
    pub source_id: String,
    pub target_id: String,
    pub requests: Vec<Request>,
}

impl Into<BoltType> for Connection {
    fn into(self) -> BoltType {
        let mut map = BoltMap::new();
        map.put("source".into(), self.source_id.into());
        map.put("target".into(), self.target_id.into());
        let serialized_requests: BoltType = BoltType::List(BoltList {
            value: self
                .requests
                .into_iter()
                .map(|restcall| {
                    let restcall_json = serde_json::to_string(&restcall).unwrap();
                    BoltType::String(BoltString::new(&restcall_json))
                })
                .collect(),
        });
        map.put("requests".into(), serialized_requests);
        BoltType::Map(map)
    }
}

impl TryFrom<BoltMap> for Connection {
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

        let requests = match node.get("requests") {
            Ok(BoltType::List(l)) => l
                .value
                .into_iter()
                .map(|request| match request {
                    BoltType::String(json_field) => Ok(serde_json::from_str(
                        json_field.value.as_str(),
                    )
                    .unwrap_or(Request {
                        endpoint: Endpoint::default(),
                        restcall: RestCall::default(),
                    })),
                    _ => Err(DeError::InvalidType {
                        received: Unexpected::Other("Non BoltString type").into(),
                        expected: "BoltString".to_string(),
                    }),
                })
                .collect::<Result<Vec<Request>, DeError>>()?,
            _ => {
                return Err(DeError::Other(
                    "Requests argument not present in Connection".to_string(),
                ));
            }
        };

        Ok(Connection {
            source_id,
            target_id,
            requests,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, PartialEq, Eq)]
pub struct Request {
    pub endpoint: Endpoint,
    pub restcall: RestCall,
}

impl Into<BoltType> for Request {
    fn into(self) -> BoltType {
        let mut map = BoltMap::new();
        let endpoint_json = serde_json::to_string(&self.endpoint).unwrap();
        map.put(
            "endpoint".into(),
            BoltType::String(BoltString::new(&endpoint_json)),
        );

        let restcall_json = serde_json::to_string(&self.restcall).unwrap();
        map.put(
            "restcall".into(),
            BoltType::String(BoltString::new(&restcall_json)),
        );
        BoltType::Map(map)
    }
}

impl TryFrom<BoltMap> for Request {
    type Error = DeError;

    fn try_from(node: BoltMap) -> Result<Self, Self::Error> {
        let endpoint = match node.get("endpoint") {
            Ok(BoltType::String(ser_endpoint)) => Ok(serde_json::from_str(
                ser_endpoint.value.as_str(),
            )
            .unwrap_or(Endpoint {
                file_path: "Bad Deserialization!".to_string(),
                function_name: "Bad Deserialization!".to_string(),
                http_method: models::HttpMethod::DELETE,
                parameters: vec![],
                uri: "Bad Deserialization!".to_string(),
            })),
            _ => Err(DeError::InvalidType {
                received: Unexpected::Other("Non BoltString type").into(),
                expected: "BoltString".to_string(),
            }),
        }?;

        let restcall = match node.get("restcall") {
            Ok(BoltType::String(ser_restcall)) => Ok(serde_json::from_str(
                ser_restcall.value.as_str(),
            )
            .unwrap_or(RestCall {
                file_path: "Bad Deserialization!".to_string(),
                function_name: "Bad Deserialization!".to_string(),
                http_method: models::HttpMethod::DELETE,
                function_arguments: vec![],
                target_uri: "Bad Deserialization!".to_string(),
            })),
            _ => Err(DeError::InvalidType {
                received: Unexpected::Other("Non BoltString type").into(),
                expected: "BoltString".to_string(),
            }),
        }?;

        Ok(Request { endpoint, restcall })
    }
}
