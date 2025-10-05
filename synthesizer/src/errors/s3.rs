use s3::error::S3Error;
use thiserror::Error;

use crate::errors::service::ServiceError;

#[derive(Debug, Error)]
pub enum S3ClientError {
    #[error("Serialization error: {0}")]
    Serde(String),

    #[error("S3 client error: {0}")]
    S3(String),
}

#[derive(thiserror::Error, Debug)]
pub enum S3ServiceError {
    #[error("can't deserialize: {0}")]
    DeserializationError(String),

    #[error("validation failed: {0}")]
    ValidationError(String),

    #[error("operation not permitted: {0}")]
    Forbidden(String),

    #[error("unexpected internal error: {0}")]
    InternalError(String),
}

impl From<ServiceError> for S3ServiceError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::DeserializationError(err) => S3ServiceError::DeserializationError(err),
            ServiceError::ValidationError(err) => S3ServiceError::ValidationError(err),
            ServiceError::Forbidden(err) => S3ServiceError::Forbidden(err),
            ServiceError::InternalError(err) => S3ServiceError::InternalError(err),
        }
    }
}

// S3ClientError` needs to implement `From<S3Error>

impl From<S3Error> for S3ClientError {
    fn from(err: S3Error) -> Self {
        match err {
            S3Error::UrlParse(err) => S3ClientError::Serde(err.to_string()),
            S3Error::FromUtf8(err) => S3ClientError::Serde(err.to_string()),
            S3Error::SerdeXml(err) => S3ClientError::Serde(err.to_string()),
            S3Error::SerdeError(err) => S3ClientError::Serde(err.to_string()),
            S3Error::XmlSeError(err) => S3ClientError::Serde(err.to_string()),
            err => S3ClientError::S3(err.to_string()),
        }
    }
}

impl From<serde_json::Error> for S3ClientError {
    fn from(err: serde_json::Error) -> Self {
        S3ClientError::Serde(err.to_string())
    }
}

impl From<S3ClientError> for S3ServiceError {
    fn from(err: S3ClientError) -> Self {
        match err {
            S3ClientError::Serde(err) => S3ServiceError::DeserializationError(err),
            S3ClientError::S3(err) => S3ServiceError::InternalError(err),
        }
    }
}
