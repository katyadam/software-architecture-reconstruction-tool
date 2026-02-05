use std::ops::DerefMut;

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{QueryDsl, RunQueryDsl, SelectableHelper, delete};
use uuid::Uuid;

use crate::error::DatabaseError;
use crate::model::{Commit, Constant, NewConstant};
use crate::schema;
use crate::schema::commits::dsl::*;
use crate::schema::constants::dsl::*;

pub trait ConstantRepository {
    fn get_single(&self, uuid_to_get: Uuid) -> Result<Constant, DatabaseError>;
    fn save(&self, new_constant: NewConstant) -> Result<Constant, DatabaseError>;
    fn save_batch(&self, new_constants: &[NewConstant]) -> Result<Vec<Constant>, DatabaseError>;
    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), DatabaseError>;
}

#[derive(Clone)]
pub struct PgConstantRepository {
    pg_pool: Pool<ConnectionManager<PgConnection>>,
}

impl PgConstantRepository {
    pub fn new(pg_pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        Self { pg_pool }
    }
}

impl ConstantRepository for PgConstantRepository {
    fn get_single(&self, uuid_to_get: Uuid) -> Result<Constant, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let constant = constants
            .find(uuid_to_get)
            .select(Constant::as_select())
            .first(conn.deref_mut())?;

        Ok(constant)
    }

    fn save(&self, new_constant: NewConstant) -> Result<Constant, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let constant = conn.deref_mut().transaction(|conn| {
            commits
                .find(new_constant.commit_hash.clone())
                .select(Commit::as_select())
                .first(conn)?;

            diesel::insert_into(schema::constants::table)
                .values(new_constant)
                .returning(Constant::as_returning())
                .get_result(conn)
        })?;

        Ok(constant)
    }

    fn save_batch(&self, new_constants: &[NewConstant]) -> Result<Vec<Constant>, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let saved_constants = conn.deref_mut().transaction(|conn| {
            if let Some(first) = new_constants.first() {
                commits
                    .find(&first.commit_hash)
                    .select(Commit::as_select())
                    .first(conn)?;
            }

            diesel::insert_into(schema::constants::table)
                .values(new_constants)
                .returning(Constant::as_returning())
                .get_results(conn)
        })?;

        Ok(saved_constants)
    }

    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        conn.deref_mut().transaction(|conn| {
            constants
                .find(uuid_to_delete)
                .select(Constant::as_select())
                .first(conn)?;

            delete(constants.filter(constant_uuid.eq(uuid_to_delete))).execute(conn)
        })?;

        Ok(())
    }
}
