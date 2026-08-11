pub mod bolt;

use models::{Endpoint, MessageEdge, RestCall, configuration::ServiceDescription};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(ToSchema, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sdg {
    pub services: Vec<Service>,
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub message_connections: Vec<MessageConnection>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, PartialEq, Eq)]
pub struct Service {
    pub name: String,
    pub endpoints: Vec<Endpoint>,
    pub urls: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, PartialEq, Eq)]
pub struct Connection {
    pub source_id: String,
    pub target_id: String,
    pub requests: Vec<Request>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, PartialEq, Eq)]
pub struct Request {
    pub endpoint: Endpoint,
    pub restcall: RestCall,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, PartialEq, Eq)]
pub struct MessageConnection {
    pub source_id: String,
    pub target_id: String,
    pub messages: Vec<MessageRequest>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, PartialEq, Eq)]
pub struct MessageRequest {
    pub producer: MessageEdge,
    pub consumer: MessageEdge,
}

#[derive(Debug, Clone)]
pub struct AssignedEndpoint {
    pub data: Endpoint,
    pub service: ServiceDescription,
}

impl AssignedEndpoint {
    pub fn new(data: Endpoint, service: ServiceDescription) -> Self {
        Self { data, service }
    }
}

#[derive(Debug, Clone)]
pub struct AssignedRestCall {
    pub data: RestCall,
    pub service: ServiceDescription,
}

impl AssignedRestCall {
    pub fn new(data: RestCall, service: ServiceDescription) -> Self {
        Self { data, service }
    }
}

#[derive(Debug, Clone)]
pub struct AssignedMessageEdge {
    pub data: MessageEdge,
    pub service: ServiceDescription,
}

impl AssignedMessageEdge {
    pub fn new(data: MessageEdge, service: ServiceDescription) -> Self {
        Self { data, service }
    }
}
