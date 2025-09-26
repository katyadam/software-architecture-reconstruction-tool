use actix_web::{HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

use crate::errors::database::DatabaseError;

// User Facing Errors
#[derive(Error, Debug, Serialize, Deserialize, ToSchema)]
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
