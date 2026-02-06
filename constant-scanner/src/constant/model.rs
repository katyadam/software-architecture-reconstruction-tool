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
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::constants)]
pub struct NewConstant {
    pub constant_uuid: Uuid,
    pub name: String,
    pub value: String,
    pub commit_hash: String,
    pub created_at: DateTime<Utc>,
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
