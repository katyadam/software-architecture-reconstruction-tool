use chrono::{DateTime, Utc};
use diesel::Queryable;
use diesel::prelude::*;
use uuid::Uuid;

#[derive(Queryable, Insertable, Debug, Selectable)]
#[diesel(table_name = crate::schema::constants)]
pub struct Constant {
    pub constant_uuid: Uuid,
    pub name: String,
    pub value: String,
    pub commit_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Queryable, Insertable, Debug, Selectable)]
#[diesel(table_name = crate::schema::commits)]
pub struct Commit {
    pub commit_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::codebases)]
pub struct NewCodebase {
    pub codebase_uuid: Uuid,
    pub branch: String,
    pub project_uuid: Uuid,
    pub created_at: DateTime<Utc>,
}

impl Codebase {
    pub fn to_response(&self) -> CodebaseResponse {
        CodebaseResponse {
            codebase_uuid: self.codebase_uuid,
            branch: self.branch.clone(),
            project_uuid: self.project_uuid,
            created_at: self.created_at.to_rfc3339(),
        }
    }
}
