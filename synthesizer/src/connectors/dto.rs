use chrono::{DateTime, Utc};
use models::ConfigurationData;
use serde::Deserialize;
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct ConfigurationDto {
    pub configuration_uuid: Uuid,
    pub configuration_data: ConfigurationData,
    pub created_at: DateTime<Utc>,
}
