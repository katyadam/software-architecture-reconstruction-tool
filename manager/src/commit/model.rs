use chrono::{DateTime, Utc};
use diesel::Queryable;
use diesel::prelude::*;
use uuid::Uuid;

use crate::commit::dto::CommitResponse;

#[derive(Queryable, Insertable, Debug, Selectable)]
#[diesel(table_name = crate::schema::commits)]
pub struct Commit {
    pub commit_hash: String,
    pub commit_message: String,
    pub codebase_uuid: Uuid,
    pub created_at: DateTime<Utc>,
    pub processed: bool,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::commits)]
pub struct NewCommit {
    pub commit_hash: String,
    pub commit_message: String,
    pub codebase_uuid: Uuid,
    pub created_at: DateTime<Utc>,
    pub processed: bool,
}

impl Commit {
    pub fn to_response(&self) -> CommitResponse {
        CommitResponse {
            commit_hash: self.commit_hash.clone(),
            commit_message: self.commit_message.clone(),
            codebase_uuid: self.codebase_uuid,
            created_at: self.created_at.to_rfc3339(),
            processed: self.processed,
        }
    }
}
