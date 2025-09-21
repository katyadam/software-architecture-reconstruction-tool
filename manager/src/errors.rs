use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

/// Internal database layer error type
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

/// User facing error type
#[derive(Error, Debug, Serialize, ToSchema)]
pub enum ApiError {
    #[error("internal server error")]
    InternalServerError,
    #[error("not found")]
    NotFound,
    #[error("bad request")]
    BadRequest,
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
        }
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            ApiError::InternalServerError => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::NotFound => actix_web::http::StatusCode::NOT_FOUND,
            ApiError::BadRequest => actix_web::http::StatusCode::BAD_REQUEST,
        }
    }
}
