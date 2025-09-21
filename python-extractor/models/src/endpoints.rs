use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::HttpMethod;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema, Clone, Default)]
pub struct Endpoint {
    pub function_name: String,
    pub http_method: HttpMethod,
    pub parameters: Vec<String>,
    pub uri: String,
    pub service_name: String,
}
