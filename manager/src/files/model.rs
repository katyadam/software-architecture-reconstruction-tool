use chrono::DateTime;
use chrono::Utc;
use diesel::Queryable;
use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::file_records;

#[derive(Queryable, Selectable)]
#[diesel(table_name = file_records)]
pub struct FileRecord {
    id: i64,
    file_path: String,
    codebase_uuid: Uuid,
    uploaded_at: DateTime<Utc>,
    file_size: i64,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::file_records)]
pub struct NewFileRecord {
    pub codebase_uuid: Uuid,
    pub file_path: String,
    pub file_size: i64,
    pub uploaded_at: DateTime<Utc>,
}
