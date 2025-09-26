use uuid::Uuid;

use crate::{
    codebase::{
        model::{Codebase, NewCodebase},
        repository::CodebaseRepository,
    },
    configuration::model::Configuration,
    errors::service::ServiceError,
};

pub trait CodebaseService {
    fn get_single(&self, uuid_to_find: Uuid) -> Result<Codebase, ServiceError>;
    fn create(&self, new_codebase: NewCodebase) -> Result<Codebase, ServiceError>;
    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), ServiceError>;
    fn get_codebase_configuration(
        &self,
        codebase_uuid: Uuid,
    ) -> Result<Configuration, ServiceError>;
}

pub struct CodebaseServiceImpl {
    repository: Box<dyn CodebaseRepository>,
}

impl CodebaseServiceImpl {
    pub fn new(repository: Box<dyn CodebaseRepository>) -> Self {
        Self { repository }
    }
}

impl CodebaseService for CodebaseServiceImpl {
    fn get_single(&self, uuid_to_find: Uuid) -> Result<Codebase, ServiceError> {
        let codebase = self.repository.get_single(uuid_to_find)?;
        Ok(codebase)
    }

    fn create(&self, new_codebase: NewCodebase) -> Result<Codebase, ServiceError> {
        let created_codebase = self.repository.save(new_codebase)?;
        Ok(created_codebase)
    }

    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), ServiceError> {
        self.repository.delete(uuid_to_delete)?;
        Ok(())
    }

    fn get_codebase_configuration(
        &self,
        codebase_uuid: Uuid,
    ) -> Result<Configuration, ServiceError> {
        let codebase_configuration = self.repository.get_codebase_configuration(codebase_uuid)?;
        Ok(codebase_configuration)
    }
}
