use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConfigurationData {
    pub service_descriptions: Vec<ServiceDescription>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Default, Clone)]
pub struct ServiceDescription {
    pub name: String,
    pub base_dir_path: String,
    pub urls: Vec<String>,
}
