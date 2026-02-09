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

impl RestCall {
    pub fn clone_from_target_uri(&self, cloned_target_uri: &str) -> Self {
        RestCall {
            function_name: self.function_name.clone(),
            function_hash: self.function_hash.clone(),
            call_arguments: self.call_arguments.clone(),
            http_method: self.http_method.clone(),
            target_uri: cloned_target_uri.to_owned(),
            file_path: self.file_path.clone(),
        }
    }

    pub fn clone_from_target_uri_helper(&self, cloned_target_uri: &str) -> Self {
        RestCall {
            function_name: self.function_name.clone(),
            function_hash: self.function_hash.clone(),
            call_arguments: self.call_arguments.clone(),
            http_method: self.http_method.clone(),
            target_uri: cloned_target_uri.to_owned(),
            file_path: self.file_path.clone(),
        }
    }
}
