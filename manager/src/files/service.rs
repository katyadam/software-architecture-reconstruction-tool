use crate::{
    errors::service::ServiceError,
    files::{
        model::{FileRecord, NewFileRecord},
        repository::FileRecordsRepository,
    },
};

pub trait FileRecordsService {
    fn create(&self, new_record: NewFileRecord) -> Result<FileRecord, ServiceError>;
}

pub struct FileRecordsServiceImpl {
    repository: Box<dyn FileRecordsRepository>,
}

impl FileRecordsServiceImpl {
    pub fn new(repository: Box<dyn FileRecordsRepository>) -> Self {
        Self { repository }
    }
}

impl FileRecordsService for FileRecordsServiceImpl {
    fn create(&self, new_record: NewFileRecord) -> Result<FileRecord, ServiceError> {
        let created_record = self.repository.save(new_record)?;
        Ok(created_record)
    }
}
