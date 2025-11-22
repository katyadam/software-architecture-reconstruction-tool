use crate::{
    commit::{
        model::{Commit, NewCommit},
        repository::CommitRepository,
    },
    errors::service::ServiceError,
};

pub trait CommitService {
    fn get_single(&self, hash_to_find: String) -> Result<Commit, ServiceError>;
    fn create(&self, new_commit: NewCommit) -> Result<Commit, ServiceError>;
    fn delete(&self, hash_to_delete: String) -> Result<(), ServiceError>;
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
}
