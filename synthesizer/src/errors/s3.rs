use s3::error::S3Error;
use thiserror::Error;

use crate::errors::api::ApiError;
#[derive(Debug, Error)]
pub enum S3ClientError {
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("S3 client error: {0}")]
    S3(#[from] S3Error),
}

impl From<S3ClientError> for ApiError {
    fn from(err: S3ClientError) -> Self {
        match err {
            S3ClientError::Serde(_) => ApiError::InternalServerError,
            S3ClientError::S3(err) => ApiError::OtherServerResponseError(err.to_string()),
        }
    }
}
