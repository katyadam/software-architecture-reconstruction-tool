use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostConfiguration {
    pub configuration_data: Value,
    #[schema(example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    pub project_uuid: Uuid,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateConfiguration {
    pub new_configuration_data: Value,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConfigurationResponse {
    pub configuration_uuid: Uuid,
    pub project_uuid: Uuid,
    pub configuration_data: Value,
    pub created_at: String,
}
