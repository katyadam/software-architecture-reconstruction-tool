use chrono::Utc;
use uuid::Uuid;

use crate::{
    api::{dto::ConstantBatchInput, repository::ConstantRepository},
    error::ServiceError,
    model::{Constant, NewConstant},
};

pub trait ConstantService {
    fn get_single(&self, uuid_to_get: Uuid) -> Result<Constant, ServiceError>;
    fn create(&self, new_constant: NewConstant) -> Result<Constant, ServiceError>;
    fn create_batch_from_keyvalues(
        &self,
        batch: ConstantBatchInput,
    ) -> Result<Vec<Constant>, ServiceError>;
    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), ServiceError>;
}

pub struct ConstantServiceImpl {
    repository: Box<dyn ConstantRepository>,
}

impl ConstantServiceImpl {
    pub fn new(repository: Box<dyn ConstantRepository>) -> Self {
        Self { repository }
    }
}

impl ConstantService for ConstantServiceImpl {
    fn get_single(&self, uuid_to_find: Uuid) -> Result<Constant, ServiceError> {
        let conf = self.repository.get_single(uuid_to_find)?;
        Ok(conf)
    }

    fn create_batch_from_keyvalues(
        &self,
        batch: ConstantBatchInput,
    ) -> Result<Vec<Constant>, ServiceError> {
        let now = Utc::now();

        let new_constants = batch
            .constants
            .into_iter()
            .map(|constant| NewConstant {
                constant_uuid: Uuid::new_v4(),
                name: constant.name,
                value: constant.value,
                commit_hash: batch.commit_hash.clone(),
                created_at: now,
            })
            .collect::<Vec<NewConstant>>();

        let created_constants = self.repository.save_batch(&new_constants)?;

        Ok(created_constants)
    }

    fn create(&self, new_constant: NewConstant) -> Result<Constant, ServiceError> {
        let created_constant = self.repository.save(new_constant)?;
        Ok(created_constant)
    }

    fn delete(&self, uuid_to_delete: Uuid) -> Result<(), ServiceError> {
        self.repository.delete(uuid_to_delete)?;
        Ok(())
    }
}
