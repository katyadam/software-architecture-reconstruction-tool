use std::ops::DerefMut;

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{QueryDsl, RunQueryDsl, SelectableHelper, delete};
use uuid::Uuid;

use crate::errors::DatabaseError;
use crate::project::model::{NewProject, Project};
use crate::schema;
use crate::schema::projects::dsl::*;

pub trait ProjectRepository {
    fn get_single(&self, uuid_to_find: Uuid) -> Result<Project, DatabaseError>;
    fn save(&self, new_project: NewProject) -> Result<Project, DatabaseError>;
    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), DatabaseError>;
}

#[derive(Clone)]
pub struct PgProjectRepository {
    pg_pool: Pool<ConnectionManager<PgConnection>>,
}

impl PgProjectRepository {
    pub fn new(pg_pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        Self { pg_pool }
    }
}

impl ProjectRepository for PgProjectRepository {
    fn get_single(&self, uuid_to_find: Uuid) -> Result<Project, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let project = projects
            .find(uuid_to_find)
            .select(Project::as_select())
            .first(conn.deref_mut())?;

        Ok(project)
    }

    fn save(&self, new_project: NewProject) -> Result<Project, DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        let project = diesel::insert_into(schema::projects::table)
            .values(new_project)
            .returning(Project::as_returning())
            .get_result(conn.deref_mut())?;

        Ok(project)
    }

    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), DatabaseError> {
        let mut conn = self.pg_pool.get()?;

        conn.deref_mut().transaction(|conn| {
            projects
                .find(uuid_to_delete)
                .select(Project::as_select())
                .first(conn)?;

            delete(projects.filter(project_uuid.eq(uuid_to_delete))).execute(conn)
        })?;

        Ok(())
    }
}
