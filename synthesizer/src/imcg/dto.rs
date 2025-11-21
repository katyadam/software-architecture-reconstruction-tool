use models::{CallStatement, Callable};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::imcg::model::Imcg;
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostIMCG {
    #[schema(example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    pub codebase_uuid: Uuid,

    #[schema(example = json!([
        {
            "signature": "unique signature of the callable",
            "namespace": "namespace where the callable lives",
            "parameters": [
                "skip", "limit", "q"
            ],
            "return_type": "int",
            "is_async": true,
            "is_constructor": false,
            "hash": "qwertzasdf"
        }
    ]))]
    pub callables: Vec<Callable>,

    #[schema(example = json!([
        {
            "function_name": "get_items",
            "arguments": [
                {
                    "assigned_variable": "params",
                    "value": "{\"skip\": skip, \"limit\": limit}"
                }
            ],
            "enclosing_function_name": "main",
            "enclosing_class_name": "class",
        }
    ]))]
    pub call_statements: Vec<CallStatement>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct PostIMCGErrorResponse {
    #[schema(example = "IMCG created but not saved.")]
    pub warning: String,
    #[schema(example = "Some Error occured.")]
    pub error: String,

    pub imcg: Imcg,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct GetIMCGErrorReponse {
    #[schema(example = "Some Error occured.")]
    pub error: String,
}
