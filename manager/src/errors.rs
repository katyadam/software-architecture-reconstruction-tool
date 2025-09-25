use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

/// Repository Layer Errors
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

/// User Facing Errors
#[derive(Error, Debug, Serialize, ToSchema)]
pub enum ApiError {
    #[error("internal server error")]
    InternalServerError,

    #[error("not found")]
    NotFound,

    #[error("bad request")]
    BadRequest,

    #[error("forbidden")]
    Forbidden,
}

impl From<DatabaseError> for ApiError {
    fn from(error: DatabaseError) -> Self {
        match error {
            DatabaseError::ConnectionError(_) | DatabaseError::Error(_) => {
                Self::InternalServerError
            }
            DatabaseError::NotFound => Self::NotFound,
        }
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::InternalServerError => {
                HttpResponse::InternalServerError().json("internal server error")
            }
            ApiError::NotFound => HttpResponse::NotFound().json("not found"),
            ApiError::BadRequest => HttpResponse::BadRequest().json("bad request"),
            ApiError::Forbidden => HttpResponse::Forbidden().json("forbidden"),
        }
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            ApiError::InternalServerError => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::NotFound => actix_web::http::StatusCode::NOT_FOUND,
            ApiError::BadRequest => actix_web::http::StatusCode::BAD_REQUEST,
            ApiError::Forbidden => actix_web::http::StatusCode::FORBIDDEN,
        }
    }
}
