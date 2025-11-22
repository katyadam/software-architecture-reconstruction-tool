use neo4rs::DeError;
use thiserror::Error;

// Repository Layer Errors
#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("database error: {0}")]
    Error(neo4rs::Error),

    #[error("can't connect to graph database")]
    ConnectionError,

    #[error("failed to deserialize error: {0}")]
    DeserializationError(DeError),
}

impl From<neo4rs::Error> for DatabaseError {
    fn from(error: neo4rs::Error) -> Self {
        match error {
            neo4rs::Error::ConnectionError => DatabaseError::ConnectionError,
            err => DatabaseError::Error(err),
        }
    }
}

impl From<DeError> for DatabaseError {
    fn from(error: DeError) -> Self {
        DatabaseError::DeserializationError(error)
    }
}
