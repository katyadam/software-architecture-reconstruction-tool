use chrono::DateTime;
use chrono::Utc;
use diesel::Queryable;
use diesel::prelude::*;
use uuid::Uuid;

use crate::project::dto::ProjectResponse;

#[derive(Queryable, Insertable, Debug, Selectable)]
#[diesel(table_name = crate::schema::projects)]
pub struct Project {
    pub project_uuid: Uuid,
    pub name: String,
    pub owner: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::projects)]
pub struct NewProject {
    pub project_uuid: Uuid,
    pub name: String,
    pub owner: String,
    pub created_at: DateTime<Utc>,
}

impl Project {
    pub fn to_response(&self) -> ProjectResponse {
        ProjectResponse {
            project_uuid: self.project_uuid,
            name: self.name.clone(),
            owner: self.owner.clone(),
            created_at: self.created_at.to_rfc3339(),
        }
    }
}
