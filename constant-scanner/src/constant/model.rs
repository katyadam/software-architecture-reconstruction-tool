use chrono::{DateTime, Utc};
use diesel::Queryable;
use diesel::prelude::*;
use uuid::Uuid;

use crate::constant::dto::ConstantResponse;

#[derive(Queryable, Insertable, Debug, Selectable)]
#[diesel(table_name = crate::schema::commits)]
pub struct Commit {
    pub commit_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Queryable, Insertable, Debug, Selectable)]
#[diesel(table_name = crate::schema::constants)]
pub struct Constant {
    pub constant_uuid: Uuid,
    pub name: String,
    pub value: String,
    pub commit_hash: String,
    pub created_at: DateTime<Utc>,
    /// Provenance tag for scraped constants.
    /// Format: "scraper:dotenv:/abs/path" or "scraper:docker_compose:/abs/path".
    /// NULL for constants inserted via the batch API.
    pub source: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::constants)]
pub struct NewConstant {
    pub constant_uuid: Uuid,
    pub name: String,
    pub value: String,
    pub commit_hash: String,
    pub created_at: DateTime<Utc>,
    /// Provenance tag; see [`Constant::source`].
    pub source: Option<String>,
}

impl Constant {
    pub fn to_response(&self) -> ConstantResponse {
        ConstantResponse {
            constant_uuid: self.constant_uuid,
            name: self.name.clone(),
            value: self.value.clone(),
        }
    }
}
