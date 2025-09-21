use actix_web::{HttpResponse, ResponseError};
use awc::error::{PayloadError, SendRequestError};
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

#[allow(dead_code)]
#[derive(Error, Debug, Serialize, ToSchema)]
pub enum ApiError {
    #[error("internal server error")]
    InternalServerError,
    #[error("not found")]
    NotFound,
    #[error("bad request")]
    BadRequest,
    #[error("sending http request resulted in error")]
    OtherServerResponseError,
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::InternalServerError => {
                HttpResponse::InternalServerError().json("internal server error")
            }
            ApiError::NotFound => HttpResponse::NotFound().json("not found"),
            ApiError::BadRequest => HttpResponse::BadRequest().json("bad request"),
            ApiError::OtherServerResponseError => {
                HttpResponse::BadRequest().json("sending http request resulted in error")
            }
        }
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            ApiError::InternalServerError => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::NotFound => actix_web::http::StatusCode::NOT_FOUND,
            ApiError::BadRequest => actix_web::http::StatusCode::BAD_REQUEST,
            ApiError::OtherServerResponseError => actix_web::http::StatusCode::BAD_REQUEST,
        }
    }
}

#[derive(Debug, Error)]
pub enum HttpClientError {
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("HTTP request error: {0}")]
    HttpRequest(#[from] SendRequestError),

    #[error("Wrong Payload: {0}")]
    Payload(#[from] PayloadError),

    #[error("Api Error: {0}")]
    ApiError(#[from] ApiError),
}
