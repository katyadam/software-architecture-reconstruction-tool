use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct Configuration {
    pub configuration_uuid: Uuid,
    pub project_uuid: Uuid,
    pub configuration_data: ConfigurationData,
    pub created_at: DateTime<Utc>,
}

pub struct ConfigurationData {
    serviceDescriptions: Vec<ServiceDescription>,
}

pub struct ServiceDescription {
    name: String,
    base_dir_path: String,
    urls: Vec<String>,
}
