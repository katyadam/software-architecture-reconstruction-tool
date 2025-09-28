use uuid::Uuid;

use crate::{
    configuration::{
        model::{Configuration, NewConfiguration},
        repository::ConfigurationRepository,
    },
    errors::service::ServiceError,
};

pub trait ConfigurationService {
    fn get_single(&self, uuid_to_find: Uuid) -> Result<Configuration, ServiceError>;
    fn create(&self, new_configuration: NewConfiguration) -> Result<Configuration, ServiceError>;
    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), ServiceError>;
}

pub struct ConfigurationServiceImpl {
    repository: Box<dyn ConfigurationRepository>,
}

impl ConfigurationServiceImpl {
    pub fn new(repository: Box<dyn ConfigurationRepository>) -> Self {
        Self { repository }
    }
}

impl ConfigurationService for ConfigurationServiceImpl {
    fn get_single(&self, uuid_to_find: Uuid) -> Result<Configuration, ServiceError> {
        let conf = self.repository.get_single(uuid_to_find)?;
        Ok(conf)
    }

    fn create(&self, new_configuration: NewConfiguration) -> Result<Configuration, ServiceError> {
        let created_conf = self.repository.save(new_configuration)?;
        Ok(created_conf)
    }

    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), ServiceError> {
        self.repository.delete(uuid_to_delete)?;
        Ok(())
    }
}
