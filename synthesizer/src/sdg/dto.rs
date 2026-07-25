use models::{Endpoint, MessageEdge, RestCall};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::sdg::model::Sdg;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostSDG {
    #[schema(example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    pub codebase_uuid: Uuid,

    #[schema(example = "abc123def4567890abcdef1234567890abcdef12")]
    pub commit_hash: String,

    #[schema(example = json!([
        {
            "function_name": "read_items",
            "function_hash": "abrakadabra",
            "http_method": "GET",
            "parameters": [
                {
                    "name": "skip",
                    "datatype": "int",
                    "initial_value": "0"
                }
            ],
            "file_path": "/serviceA/endpoints.py",
            "uri": "/items/"
        }
    ]))]
    pub endpoints: Vec<Endpoint>,

    #[schema(example = json!([
        {
            "function_name": "get_items",
            "function_hash": "dabrakadabra",
            "call_arguments": [
                {
                    "assigned_variable": "params",
                    "value": "{\"skip\": skip, \"limit\": limit}",
                    "datatype": "str"
                }
            ],
            "http_method": "GET",
            "target_uri": "http://localhost:8000/items/",
            "file_path": "/serviceB/restcalls.py",
        }
    ]))]
    pub restcalls: Vec<RestCall>,

    #[serde(default)]
    pub message_edges: Vec<MessageEdge>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct PostSDGErrorResponse {
    #[schema(example = "SDG created but not saved.")]
    pub warning: String,
    #[schema(example = "Some Error occured.")]
    pub error: String,

    pub sdg: Sdg,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct GetSDGErrorReponse {
    #[schema(example = "Some Error occured.")]
    pub error: String,
}
