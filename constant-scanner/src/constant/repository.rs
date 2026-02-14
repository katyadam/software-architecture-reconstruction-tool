use std::ops::DerefMut;

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{QueryDsl, RunQueryDsl, SelectableHelper};

use crate::constant::model::{Commit, Constant, NewConstant};
use crate::error::DatabaseError;
use crate::schema;
use crate::schema::commits::dsl::*;
use crate::schema::constants::dsl::*;

pub trait ConstantRepository {
    fn get_by_commit_hash(&self, commit_hash_query: &str) -> Result<Vec<Constant>, DatabaseError>;
    fn save_batch(&self, new_constants: &[NewConstant]) -> Result<Vec<Constant>, DatabaseError>;
    fn delete_by_commit_hash(&self, commit_hash_query: &str) -> Result<(), DatabaseError>;
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
    fn get_by_commit_hash(&self, commit_hash_query: &str) -> Result<Vec<Constant>, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let results = constants
            .filter(schema::constants::dsl::commit_hash.eq(commit_hash_query))
            .select(Constant::as_select())
            .load(conn.deref_mut())?;

        Ok(results)
    }

    fn save_batch(&self, new_constants: &[NewConstant]) -> Result<Vec<Constant>, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let saved_constants = conn.deref_mut().transaction(|conn| {
            if let Some(first) = new_constants.first() {
                let new_commit = Commit {
                    commit_hash: first.commit_hash.clone(),
                    created_at: first.created_at,
                };
                diesel::insert_into(schema::commits::table)
                    .values(&new_commit)
                    .on_conflict(schema::commits::commit_hash)
                    .do_nothing()
                    .execute(conn)?;
            }

            diesel::insert_into(schema::constants::table)
                .values(new_constants)
                .returning(Constant::as_returning())
                .get_results(conn)
        })?;

        Ok(saved_constants)
    }

    fn delete_by_commit_hash(&self, commit_hash_query: &str) -> Result<(), DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        diesel::delete(commits.filter(schema::commits::dsl::commit_hash.eq(commit_hash_query)))
            .execute(conn.deref_mut())?;

        Ok(())
    }
}
