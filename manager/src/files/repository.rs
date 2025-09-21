use std::ops::DerefMut;

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{RunQueryDsl, SelectableHelper};

use crate::errors::DatabaseError;
use crate::files::model::{FileRecord, NewFileRecord};
use crate::schema;

pub trait FileRecordsRepository {
    fn save(&self, new_record: NewFileRecord) -> Result<FileRecord, DatabaseError>;
}

#[derive(Clone)]
pub struct PgFileRecordsRepository {
    pg_pool: Pool<ConnectionManager<PgConnection>>,
}

impl PgFileRecordsRepository {
    pub fn new(pg_pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        Self { pg_pool }
    }
}

impl FileRecordsRepository for PgFileRecordsRepository {
    fn save(&self, new_record: NewFileRecord) -> Result<FileRecord, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let record = diesel::insert_into(schema::file_records::table)
            .values(new_record)
            .returning(FileRecord::as_returning())
            .get_result(conn.deref_mut())?;

        Ok(record)
    }
}
