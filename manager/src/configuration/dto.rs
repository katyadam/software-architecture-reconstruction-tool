use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostConfiguration {
    pub configuration_data: ConfigurationData,
    #[schema(example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    pub project_uuid: Uuid,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConfigurationResponse {
    pub configuration_uuid: Uuid,
    pub project_uuid: Uuid,
    pub configuration_data: ConfigurationData,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConfigurationData {
    services: Vec<Service>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
struct Service {
    name: String,
    path: String,
    base_urls: Vec<String>,
}
