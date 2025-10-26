use chrono::{DateTime, Utc};
use diesel::Queryable;
use diesel::prelude::*;
use models::ConfigurationData;
use serde_json::Value;
use uuid::Uuid;

use crate::configuration::dto::ConfigurationResponse;
use crate::errors::model::ModelError;

// Please be aware that the shared struct "Configuration" is defined inside the models lib!
#[derive(Queryable, Insertable, Debug, Selectable)]
#[diesel(table_name = crate::schema::configurations)]
pub struct DbConfiguration {
    pub configuration_uuid: Uuid,
    pub project_uuid: Uuid,
    pub configuration_data: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::configurations)]
pub struct NewDbConfiguration {
    pub configuration_uuid: Uuid,
    pub project_uuid: Uuid,
    pub configuration_data: Value,
    pub created_at: DateTime<Utc>,
}

impl DbConfiguration {
    pub fn to_response(&self) -> Result<ConfigurationResponse, ModelError> {
        let configuration_data: ConfigurationData =
            serde_json::from_value(self.configuration_data.clone()).map_err(ModelError::from)?;

        Ok(ConfigurationResponse {
            configuration_uuid: self.configuration_uuid,
            project_uuid: self.project_uuid,
            configuration_data,
            created_at: self.created_at.to_rfc3339(),
        })
    }
}
