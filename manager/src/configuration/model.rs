use chrono::{DateTime, Utc};
use diesel::Queryable;
use diesel::prelude::*;
use serde_json::Value;
use uuid::Uuid;

use crate::configuration::dto::ConfigurationResponse;

#[derive(Queryable, Insertable, Debug, Selectable)]
#[diesel(table_name = crate::schema::configurations)]
pub struct Configuration {
    pub configuration_uuid: Uuid,
    pub project_uuid: Uuid,
    pub configuration_data: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::configurations)]
pub struct NewConfiguration {
    pub configuration_uuid: Uuid,
    pub project_uuid: Uuid,
    pub configuration_data: Value,
    pub created_at: DateTime<Utc>,
}

impl Configuration {
    pub fn to_response(&self) -> ConfigurationResponse {
        ConfigurationResponse {
            configuration_uuid: self.configuration_uuid,
            project_uuid: self.project_uuid,
            configuration_data: self.configuration_data.clone(),
            created_at: self.created_at.to_rfc3339(),
        }
    }
}
