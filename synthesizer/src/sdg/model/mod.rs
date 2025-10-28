pub mod bolt;

use models::{Endpoint, RestCall, configuration::ServiceDescription};
use serde::{Deserialize, Serialize};
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
