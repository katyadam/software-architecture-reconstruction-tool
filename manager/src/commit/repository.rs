use std::ops::DerefMut;

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{QueryDsl, RunQueryDsl, SelectableHelper, delete};

use crate::codebase::model::Codebase;
use crate::commit::model::{Commit, NewCommit};
use crate::errors::database::DatabaseError;
use crate::schema;
use crate::schema::codebases::dsl::*;
use crate::schema::commits::dsl::*;

pub trait CommitRepository {
    fn get_single(&self, hash_to_find: String) -> Result<Commit, DatabaseError>;
    fn save(&self, new_commit: NewCommit) -> Result<Commit, DatabaseError>;
    fn delete(&self, hash_to_delete: String) -> Result<(), DatabaseError>;
}

#[derive(Clone)]
pub struct PgCommitRepository {
    pg_pool: Pool<ConnectionManager<PgConnection>>,
}

impl PgCommitRepository {
    pub fn new(pg_pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        Self { pg_pool }
    }
}

impl CommitRepository for PgCommitRepository {
    fn get_single(&self, hash_to_find: String) -> Result<Commit, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let commit = commits
            .find(hash_to_find)
            .select(Commit::as_select())
            .first(conn.deref_mut())?;

        Ok(commit)
    }

    fn save(&self, new_commit: NewCommit) -> Result<Commit, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let commit = conn.deref_mut().transaction(|conn| {
            codebases
                .find(new_commit.codebase_uuid)
                .select(Codebase::as_select())
                .first(conn)?;

            diesel::insert_into(schema::commits::table)
                .values(new_commit)
                .returning(Commit::as_returning())
                .get_result(conn)
        })?;

        Ok(commit)
    }

    fn delete(&self, hash_to_delete: String) -> Result<(), DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        conn.deref_mut().transaction(|conn| {
            commits
                .find(hash_to_delete.clone())
                .select(Commit::as_select())
                .first(conn)?;

            delete(commits.filter(commit_hash.eq(hash_to_delete))).execute(conn)
        })?;

        Ok(())
    }
}
