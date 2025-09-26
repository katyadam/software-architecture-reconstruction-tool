use crate::errors::{api::ApiError, database::DatabaseError};

// Business Logic Errors
#[derive(thiserror::Error, Debug)]
pub enum ServiceError {
    #[error("entity not found")]
    NotFound,

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
            DatabaseError::NotFound => ServiceError::NotFound,
            DatabaseError::ConnectionError(_) | DatabaseError::Error(_) => {
                ServiceError::InternalError
            }
        }
    }
}

impl From<ServiceError> for ApiError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::NotFound => ApiError::NotFound,
            ServiceError::ValidationError(_) => ApiError::BadRequest,
            ServiceError::Forbidden(_) => ApiError::Forbidden,
            ServiceError::InternalError => ApiError::InternalServerError,
        }
    }
}
