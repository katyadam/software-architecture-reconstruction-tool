use actix_web::error;
use neo4rs::DeError;
use thiserror::Error;

// Repository Layer Errors
#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("database error: {0}")]
    Error(neo4rs::Error),

    #[error("failed to deserialize error: {0}")]
    DeserializationError(DeError),
}

impl From<neo4rs::Error> for DatabaseError {
    fn from(error: neo4rs::Error) -> Self {
        match error {
            err => DatabaseError::Error(err),
        }
    }
}

impl From<DeError> for DatabaseError {
    fn from(error: DeError) -> Self {
        match error {
            err => DatabaseError::DeserializationError(err),
        }
    }
}
