use crate::errors::{builder::BuilderError, database::DatabaseError};

// Business Logic Errors
#[derive(thiserror::Error, Debug)]
pub enum ServiceError {
    #[error("can't deserialize: {0}")]
    DeserializationError(String),

    #[error("validation failed: {0}")]
    ValidationError(String),

    #[error("operation not permitted: {0}")]
    Forbidden(String),

    #[error("unexpected internal error: {0}")]
    InternalError(String),
}

impl From<DatabaseError> for ServiceError {
    fn from(err: DatabaseError) -> Self {
        match err {
            DatabaseError::DeserializationError(err) => {
                ServiceError::DeserializationError(err.to_string())
            }
            DatabaseError::Error(err) => ServiceError::InternalError(err.to_string()),
            DatabaseError::ConnectionError => {
                ServiceError::InternalError("can't connect to DB".to_string())
            }
        }
    }
}

impl From<BuilderError> for ServiceError {
    fn from(err: BuilderError) -> Self {
        match err {
            BuilderError::Error(err) => ServiceError::InternalError(err),
        }
    }
}
