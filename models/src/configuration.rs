use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConfigurationData {
    service_descriptions: Vec<ServiceDescription>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ServiceDescription {
    name: String,
    base_dir_path: String,
    urls: Vec<String>,
}
