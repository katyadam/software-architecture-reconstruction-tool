use std::ops::DerefMut;

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{QueryDsl, RunQueryDsl, SelectableHelper, delete};
use uuid::Uuid;

use crate::codebase::model::{Codebase, NewCodebase};
use crate::configuration::model::Configuration;
use crate::errors::database::DatabaseError;
use crate::project::model::Project;
use crate::schema;
use crate::schema::codebases::dsl::*;
use crate::schema::configurations::dsl::*;
use crate::schema::projects::dsl::*;

pub trait CodebaseRepository {
    fn get_single(&self, uuid_to_find: Uuid) -> Result<Codebase, DatabaseError>;
    fn save(&self, new_codebase: NewCodebase) -> Result<Codebase, DatabaseError>;
    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), DatabaseError>;
    fn get_codebase_configuration(
        &self,
        codebase_uuid_for_configuration: Uuid,
    ) -> Result<Configuration, DatabaseError>;
}

#[derive(Clone)]
pub struct PgCodebaseRepository {
    pg_pool: Pool<ConnectionManager<PgConnection>>,
}

impl PgCodebaseRepository {
    pub fn new(pg_pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        Self { pg_pool }
    }
}

impl CodebaseRepository for PgCodebaseRepository {
    fn get_single(&self, uuid_to_find: Uuid) -> Result<Codebase, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let codebase = codebases
            .find(uuid_to_find)
            .select(Codebase::as_select())
            .first(conn.deref_mut())?;

        Ok(codebase)
    }

    fn save(&self, new_codebase: NewCodebase) -> Result<Codebase, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let codebase = conn.deref_mut().transaction(|conn| {
            projects
                .find(new_codebase.project_uuid)
                .select(Project::as_select())
                .first(conn)?;

            configurations
                .find(new_codebase.configuration_uuid)
                .select(Configuration::as_select())
                .first(conn)?;

            diesel::insert_into(schema::codebases::table)
                .values(new_codebase)
                .returning(Codebase::as_returning())
                .get_result(conn)
        })?;

        Ok(codebase)
    }

    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        conn.deref_mut().transaction(|conn| {
            codebases
                .find(uuid_to_delete)
                .select(Codebase::as_select())
                .first(conn)?;

            delete(codebases.filter(codebase_uuid.eq(uuid_to_delete))).execute(conn)
        })?;

        Ok(())
    }

    fn get_codebase_configuration(
        &self,
        codebase_uuid_for_configuration: Uuid,
    ) -> Result<Configuration, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let configuration = conn.deref_mut().transaction(|conn| {
            let codebase = codebases
                .find(codebase_uuid_for_configuration)
                .select(Codebase::as_select())
                .first(conn)?;

            configurations
                .find(codebase.configuration_uuid)
                .select(Configuration::as_select())
                .first(conn)
        })?;

        Ok(configuration)
    }
}
