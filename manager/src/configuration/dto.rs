use models::ConfigurationData;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostConfiguration {
    pub configuration_data: ConfigurationData,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConfigurationResponse {
    pub configuration_uuid: Uuid,
    pub configuration_data: ConfigurationData,
    pub created_at: String,
}
