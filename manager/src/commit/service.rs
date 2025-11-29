use crate::{
    commit::{
        model::{Commit, NewCommit},
        repository::CommitRepository,
    },
    configuration::model::DbConfiguration,
    errors::service::ServiceError,
};

pub trait CommitService {
    fn get_single(&self, hash_to_find: String) -> Result<Commit, ServiceError>;
    fn create(&self, new_commit: NewCommit) -> Result<Commit, ServiceError>;
    fn delete(&self, hash_to_delete: String) -> Result<(), ServiceError>;
    fn commit_processed(&self, hash_to_set_processed: String) -> Result<(), ServiceError>;
    fn get_commit_configuration(&self, commit_hash: &str) -> Result<DbConfiguration, ServiceError>;
}

pub struct CommitServiceImpl {
    repository: Box<dyn CommitRepository>,
}

impl CommitServiceImpl {
    pub fn new(repository: Box<dyn CommitRepository>) -> Self {
        Self { repository }
    }
}

impl CommitService for CommitServiceImpl {
    fn get_single(&self, hash_to_find: String) -> Result<Commit, ServiceError> {
        let commit = self.repository.get_single(hash_to_find)?;
        Ok(commit)
    }

    fn create(&self, new_commit: NewCommit) -> Result<Commit, ServiceError> {
        let created_commit = self.repository.save(new_commit)?;
        Ok(created_commit)
    }

    fn delete(&self, hash_to_delete: String) -> Result<(), ServiceError> {
        self.repository.delete(hash_to_delete)?;
        Ok(())
    }

    fn commit_processed(&self, hash_to_set_processed: String) -> Result<(), ServiceError> {
        self.repository.commit_processed(hash_to_set_processed)?;
        Ok(())
    }

    fn get_commit_configuration(&self, commit_hash: &str) -> Result<DbConfiguration, ServiceError> {
        let codebase_configuration = self.repository.get_commit_configuration(commit_hash)?;
        Ok(codebase_configuration)
    }
}
