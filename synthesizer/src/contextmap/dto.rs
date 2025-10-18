use models::Entity;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::contextmap::model::ContextMap;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostContextMap {
    #[schema(example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    pub codebase_uuid: Uuid,

    #[schema(example = json!([
        {
            "name": "Class",
            "signature": "./entitity.py/Class",
            "superclasses": [],
            "service_name": "test_component",
            "fields": [
                {
                    "name": "entity",
                    "datatype": "Entity",
                    "datatype_signature": "./entities.py/Entity",
                    "initial_value": null
                }
            ]
        },
        {
            "name": "Entity",
            "signature": "./entities.py/Entity",
            "superclasses": [],
            "service_name": "test_component",
            "fields": [
                {
                    "name": "number",
                    "datatype": "int",
                    "datatype_signature": null,
                    "initial_value": null
                }
            ]
        }
    ]))]
    pub entities: Vec<Entity>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct PostContextMapErrorResponse {
    #[schema(example = "Context Map created but not saved.")]
    pub warning: String,
    #[schema(example = "Some Error occured.")]
    pub error: String,

    pub context_map: ContextMap,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct GetContextMapErrorReponse {
    #[schema(example = "Some Error occured.")]
    pub error: String,
}
