use chrono::{DateTime, Utc};
use diesel::Queryable;
use diesel::prelude::*;
use serde_json::Value;
use uuid::Uuid;

#[derive(Queryable, Insertable, Debug, Selectable)]
#[diesel(table_name = crate::schema::configurations)]
pub struct Configuration {
    pub configuration_uuid: Uuid,
    pub codebase_uuid: Uuid,
    pub configuration_data: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::codebases)]
pub struct NewConfiguration {
    pub codebase_uuid: Uuid,
    pub project_uuid: Uuid,
    pub created_at: DateTime<Utc>,
}
