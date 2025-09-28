use uuid::Uuid;

use crate::{
    errors::service::ServiceError,
    project::{
        model::{NewProject, Project},
        repository::ProjectRepository,
    },
};

pub trait ProjectService {
    fn get_single(&self, uuid_to_find: Uuid) -> Result<Project, ServiceError>;
    fn create(&self, new_project: NewProject) -> Result<Project, ServiceError>;
    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), ServiceError>;
}

pub struct ProjectServiceImpl {
    repository: Box<dyn ProjectRepository>,
}

impl ProjectServiceImpl {
    pub fn new(repository: Box<dyn ProjectRepository>) -> Self {
        Self { repository }
    }
}

impl ProjectService for ProjectServiceImpl {
    fn get_single(&self, uuid_to_find: Uuid) -> Result<Project, ServiceError> {
        let project = self.repository.get_single(uuid_to_find)?;
        Ok(project)
    }

    fn create(&self, new_project: NewProject) -> Result<Project, ServiceError> {
        let created_project = self.repository.save(new_project)?;
        Ok(created_project)
    }

    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), ServiceError> {
        self.repository.delete(uuid_to_delete)?;
        Ok(())
    }
}
