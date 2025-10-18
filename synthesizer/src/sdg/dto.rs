use models::{Endpoint, RestCall};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::sdg::model::SDG;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostSDG {
    #[schema(example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    pub codebase_uuid: Uuid,

    #[schema(example = json!([
        {
            "function_name": "read_items",
            "http_method": "GET",
            "parameters": [
                "skip", "limit", "q"
            ],
            "service_name": "ItemService",
            "uri": "/items/"
        }
    ]))]
    pub endpoints: Vec<Endpoint>,

    #[schema(example = json!([
        {
            "function_name": "get_items",
            "function_arguments": [
                {
                    "assigned_variable": "params",
                    "value": "{\"skip\": skip, \"limit\": limit}"
                }
            ],
            "http_method": "GET",
            "target_uri": "http://localhost:8000/items/",
            "service_name": "OrderService",
        }
    ]))]
    pub restcalls: Vec<RestCall>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct PostSDGErrorResponse {
    #[schema(example = "SDG created but not saved.")]
    pub warning: String,
    #[schema(example = "Some Error occured.")]
    pub error: String,

    pub sdg: SDG,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct GetSDGErrorReponse {
    #[schema(example = "Some Error occured.")]
    pub error: String,
}
