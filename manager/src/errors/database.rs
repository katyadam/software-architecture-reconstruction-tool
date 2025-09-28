use thiserror::Error;

// Repository Layer Errors
#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("database error: {0}")]
    Error(diesel::result::Error),

    #[error("connection pool error: {0}")]
    ConnectionError(#[from] r2d2::Error),

    #[error("no record found")]
    NotFound,
}

impl From<diesel::result::Error> for DatabaseError {
    fn from(error: diesel::result::Error) -> Self {
        match error {
            diesel::result::Error::NotFound => DatabaseError::NotFound,
            err => DatabaseError::Error(err),
        }
    }
}
