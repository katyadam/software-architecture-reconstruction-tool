use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{Argument, HttpMethod};

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Clone, Default)]
pub struct RestCall {
    pub function_name: String,
    pub function_hash: String,
    pub call_arguments: Vec<Argument>,
    pub http_method: HttpMethod,
    pub target_uri: String,
    pub file_path: String,
}
