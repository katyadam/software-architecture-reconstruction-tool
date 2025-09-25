use crate::configuration::model::{Configuration, NewConfiguration};
use crate::project::model::Project;
use crate::schema;
use std::ops::DerefMut;

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{QueryDsl, RunQueryDsl, SelectableHelper, delete};
use uuid::Uuid;

use crate::errors::DatabaseError;
use crate::schema::configurations::dsl::*;
use crate::schema::projects::dsl::*;

pub trait ConfigurationRepository {
    fn get_single(&self, uuid_to_find: Uuid) -> Result<Configuration, DatabaseError>;
    fn save(&self, new_configuration: NewConfiguration) -> Result<Configuration, DatabaseError>;
    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), DatabaseError>;
}

pub struct PgConfigurationRepository {
    pg_pool: Pool<ConnectionManager<PgConnection>>,
}

impl PgConfigurationRepository {
    pub fn new(pg_pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        Self { pg_pool }
    }
}

impl ConfigurationRepository for PgConfigurationRepository {
    fn get_single(&self, uuid_to_find: Uuid) -> Result<Configuration, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let configuration = configurations
            .find(uuid_to_find)
            .select(Configuration::as_select())
            .first(conn.deref_mut())?;

        Ok(configuration)
    }

    fn save(&self, new_configuration: NewConfiguration) -> Result<Configuration, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let configuration = conn.deref_mut().transaction(|conn| {
            projects
                .find(new_configuration.project_uuid)
                .select(Project::as_select())
                .first(conn)?;

            diesel::insert_into(schema::configurations::table)
                .values(new_configuration)
                .returning(Configuration::as_returning())
                .get_result(conn)
        })?;

        Ok(configuration)
    }

    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), DatabaseError> {
        let mut conn = self.pg_pool.get()?;
        conn.deref_mut().transaction(|conn| {
            configurations
                .find(uuid_to_delete)
                .select(Configuration::as_select())
                .first(conn)?;

            delete(configurations.filter(configuration_uuid.eq(uuid_to_delete))).execute(conn)
        })?;

        Ok(())
    }
}
