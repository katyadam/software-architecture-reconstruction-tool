use models::{Endpoint, RestCall};
use neo4rs::{BoltList, BoltMap, BoltNode, BoltString, BoltType, DeError};
use serde::de::Unexpected;

use crate::sdg::model::{Connection, MessageConnection, MessageRequest, Request, Service};

impl From<Service> for BoltType {
    fn from(value: Service) -> Self {
        let mut map = BoltMap::new();

        map.put("name".into(), value.name.into());

        let serialized_endpoints: BoltType = BoltType::List(BoltList {
            value: value
                .endpoints
                .into_iter()
                .map(|endpoint| {
                    let endpoint_json = serde_json::to_string(&endpoint).unwrap();
                    BoltType::String(BoltString::new(&endpoint_json))
                })
                .collect(),
        });
        map.put("endpoints".into(), serialized_endpoints);

        let converted_urls: BoltType = BoltType::List(BoltList {
            value: value
                .urls
                .into_iter()
                .map(|url| BoltType::String(BoltString::new(&url)))
                .collect(),
        });
        map.put("urls".into(), converted_urls);

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
                        function_hash: "Bad Deserialization!".to_string(),
                        file_path: "Bad Deserialization!".to_string(),
                        function_name: "Bad Deserialization!".to_string(),
                        http_method: models::HttpMethod::DELETE,
                        parameters: vec![],
                        ..Default::default()
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

        let urls = match node.get("urls") {
            Ok(BoltType::List(l)) => l
                .value
                .into_iter()
                .map(|url| match url {
                    BoltType::String(s) => Ok(s.value),
                    _ => Err(DeError::InvalidType {
                        received: Unexpected::Other("Non BoltString type").into(),
                        expected: "BoltString".to_string(),
                    }),
                })
                .collect::<Result<Vec<String>, DeError>>()?,
            _ => {
                return Err(DeError::Other(
                    "Urls argument not present in Service".to_string(),
                ));
            }
        };

        Ok(Service {
            name,
            endpoints,
            urls,
        })
    }
}

impl From<Connection> for BoltType {
    fn from(value: Connection) -> Self {
        let mut map = BoltMap::new();
        map.put("source_id".into(), value.source_id.into());
        map.put("target_id".into(), value.target_id.into());
        let serialized_requests: BoltType = BoltType::List(BoltList {
            value: value
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
            Err(e) => {
                return Err(DeError::Other(e.to_string()));
            }
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

impl From<MessageConnection> for BoltType {
    fn from(value: MessageConnection) -> Self {
        let mut map = BoltMap::new();
        map.put("source_id".into(), value.source_id.into());
        map.put("target_id".into(), value.target_id.into());
        let serialized_messages: BoltType = BoltType::List(BoltList {
            value: value
                .messages
                .into_iter()
                .map(|message| {
                    let message_json = serde_json::to_string(&message).unwrap();
                    BoltType::String(BoltString::new(&message_json))
                })
                .collect(),
        });
        map.put("messages".into(), serialized_messages);
        BoltType::Map(map)
    }
}

impl TryFrom<BoltMap> for MessageConnection {
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

        let messages = match node.get("messages") {
            Ok(BoltType::List(l)) => l
                .value
                .into_iter()
                .map(|message| match message {
                    BoltType::String(json_field) => {
                        serde_json::from_str::<MessageRequest>(json_field.value.as_str())
                            .map_err(|error| DeError::Other(error.to_string()))
                    }
                    _ => Err(DeError::InvalidType {
                        received: Unexpected::Other("Non BoltString type").into(),
                        expected: "BoltString".to_string(),
                    }),
                })
                .collect::<Result<Vec<MessageRequest>, DeError>>()?,
            Err(e) => return Err(DeError::Other(e.to_string())),
            _ => {
                return Err(DeError::Other(
                    "Messages argument not present in MessageConnection".to_string(),
                ));
            }
        };

        Ok(MessageConnection {
            source_id,
            target_id,
            messages,
        })
    }
}

impl From<Request> for BoltType {
    fn from(value: Request) -> Self {
        let mut map = BoltMap::new();
        let endpoint_json = serde_json::to_string(&value.endpoint).unwrap();
        map.put(
            "endpoint".into(),
            BoltType::String(BoltString::new(&endpoint_json)),
        );

        let restcall_json = serde_json::to_string(&value.restcall).unwrap();
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
                function_hash: "Bad Deserialization!".to_string(),
                function_name: "Bad Deserialization!".to_string(),
                http_method: models::HttpMethod::DELETE,
                parameters: vec![],
                uri: "Bad Deserialization!".to_string(),
                ..Default::default()
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
                function_hash: "Bad Deserialization!".to_string(),
                http_method: models::HttpMethod::DELETE,
                call_arguments: vec![],
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
