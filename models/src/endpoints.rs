use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{HttpMethod, Parameter};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema, Clone, Default)]
pub struct Endpoint {
    pub function_name: String,
    pub function_hash: String,
    pub http_method: HttpMethod,
    pub parameters: Vec<Parameter>,
    pub uri: String,
    pub file_path: String,
    pub router_variable: Option<String>,
    /// Function containing the route registration, `None` at module level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosing_function: Option<String>,
}
