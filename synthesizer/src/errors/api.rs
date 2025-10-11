use actix_web::{HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

use crate::errors::{
    database::DatabaseError,
    s3::{S3ClientError, S3ServiceError},
    service::ServiceError,
};

// User Facing Errors
#[derive(Error, Debug, Serialize, Deserialize, ToSchema)]
pub enum ApiError {
    #[error("internal server error: {0}")]
    InternalServerError(String),

    #[error("not found")]
    NotFound,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("Other server reponse error: {0}")]
    OtherServerResponseError(String),
}

impl From<DatabaseError> for ApiError {
    fn from(error: DatabaseError) -> Self {
        match error {
            DatabaseError::Error(err) => Self::InternalServerError(err.to_string()),
            DatabaseError::DeserializationError(err) => Self::BadRequest(err.to_string()),
            DatabaseError::ConnectionError => {
                Self::InternalServerError("can't connect to DB".to_string())
            }
        }
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::InternalServerError(err) => HttpResponse::InternalServerError().json(err),
            ApiError::NotFound => HttpResponse::NotFound().json("not found"),
            ApiError::BadRequest(err) => HttpResponse::BadRequest().json(err),
            ApiError::Forbidden(err) => HttpResponse::Forbidden().json(err),
            ApiError::OtherServerResponseError(err) => {
                HttpResponse::InternalServerError().json(err)
            }
        }
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            ApiError::InternalServerError(_) => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::NotFound => actix_web::http::StatusCode::NOT_FOUND,
            ApiError::BadRequest(_) => actix_web::http::StatusCode::BAD_REQUEST,
            ApiError::Forbidden(_) => actix_web::http::StatusCode::FORBIDDEN,
            ApiError::OtherServerResponseError(_) => {
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

impl From<ServiceError> for ApiError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::DeserializationError(err) => ApiError::BadRequest(err.to_string()),
            ServiceError::ValidationError(err) => ApiError::BadRequest(err),
            ServiceError::Forbidden(err) => ApiError::Forbidden(err),
            ServiceError::InternalError(err) => ApiError::InternalServerError(err),
        }
    }
}

impl From<S3ClientError> for ApiError {
    fn from(err: S3ClientError) -> Self {
        match err {
            S3ClientError::Serde(err) => ApiError::InternalServerError(err),
            S3ClientError::S3(err) => ApiError::OtherServerResponseError(err.to_string()),
        }
    }
}

impl From<S3ServiceError> for ApiError {
    fn from(err: S3ServiceError) -> Self {
        match err {
            S3ServiceError::DeserializationError(err) => ApiError::InternalServerError(err),
            S3ServiceError::ValidationError(err) => ApiError::BadRequest(err),
            S3ServiceError::Forbidden(err) => ApiError::Forbidden(err),
            S3ServiceError::InternalError(err) => ApiError::InternalServerError(err),
        }
    }
}
