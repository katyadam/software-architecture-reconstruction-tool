use chrono::Utc;
use uuid::Uuid;

use crate::{
    constant::{
        dto::ConstantBatchInput,
        model::{Constant, NewConstant},
        repository::ConstantRepository,
    },
    error::ServiceError,
};

pub trait ConstantService {
    fn get_by_commit_hash(&self, commit_hash: &str) -> Result<Vec<Constant>, ServiceError>;
    fn create_batch_from_keyvalues(
        &self,
        batch: ConstantBatchInput,
    ) -> Result<Vec<Constant>, ServiceError>;
    fn delete_by_commit_hash(&self, commit_hash: &str) -> Result<(), ServiceError>;
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
    fn get_by_commit_hash(&self, commit_hash: &str) -> Result<Vec<Constant>, ServiceError> {
        let results = self.repository.get_by_commit_hash(commit_hash)?;
        Ok(results)
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

    fn delete_by_commit_hash(&self, commit_hash: &str) -> Result<(), ServiceError> {
        self.repository.delete_by_commit_hash(commit_hash)?;
        Ok(())
    }
}
