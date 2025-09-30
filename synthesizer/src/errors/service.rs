use neo4rs::DeError;

use crate::errors::{api::ApiError, database::DatabaseError};

// Business Logic Errors
#[derive(thiserror::Error, Debug)]
pub enum ServiceError {
    #[error("can't deserialize")]
    DeserializationError(DeError),

    #[error("validation failed: {0}")]
    ValidationError(String),

    #[error("operation not permitted: {0}")]
    Forbidden(String),

    #[error("unexpected internal error")]
    InternalError,
}

impl From<DatabaseError> for ServiceError {
    fn from(err: DatabaseError) -> Self {
        match err {
            DatabaseError::DeserializationError(err) => ServiceError::DeserializationError(err),
            DatabaseError::ConnectionError | DatabaseError::Error(_) => ServiceError::InternalError,
        }
    }
}

impl From<ServiceError> for ApiError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::DeserializationError(err) => ApiError::BadRequest(err.to_string()),
            ServiceError::ValidationError(_) => ApiError::BadRequest("can't validate".to_string()),
            ServiceError::Forbidden(_) => ApiError::Forbidden,
            ServiceError::InternalError => ApiError::InternalServerError,
        }
    }
}
