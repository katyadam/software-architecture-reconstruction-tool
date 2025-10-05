use actix_web::{HttpResponse, ResponseError};
use awc::error::{PayloadError, SendRequestError};
use s3::error::S3Error;
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
    #[error("sending http request resulted in error: {0}")]
    OtherServerResponseError(String),
    #[error("can't convert file to utf-8")]
    Utf8ConversionError,
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::InternalServerError => {
                HttpResponse::InternalServerError().json("internal server error")
            }
            ApiError::NotFound => HttpResponse::NotFound().json("not found"),
            ApiError::BadRequest => HttpResponse::BadRequest().json("bad request"),
            ApiError::OtherServerResponseError(msg) => HttpResponse::BadRequest().json(msg),
            ApiError::Utf8ConversionError => {
                HttpResponse::BadRequest().json("bad request - can't convert file to utf-8")
            }
        }
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            ApiError::InternalServerError => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::NotFound => actix_web::http::StatusCode::NOT_FOUND,
            ApiError::BadRequest => actix_web::http::StatusCode::BAD_REQUEST,
            ApiError::OtherServerResponseError(_) => actix_web::http::StatusCode::BAD_REQUEST,
            ApiError::Utf8ConversionError => actix_web::http::StatusCode::BAD_REQUEST,
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
}

impl From<HttpClientError> for ApiError {
    fn from(err: HttpClientError) -> Self {
        match err {
            HttpClientError::Serde(_) => ApiError::InternalServerError,
            HttpClientError::HttpRequest(err) => {
                ApiError::OtherServerResponseError(err.to_string())
            }
            HttpClientError::Payload(_) => ApiError::BadRequest,
        }
    }
}

#[derive(Debug, Error)]
pub enum S3ClientError {
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("HTTP request error: {0}")]
    HttpRequest(#[from] SendRequestError),

    #[error("Wrong Payload: {0}")]
    Payload(#[from] PayloadError),

    #[error("S3 client error: {0}")]
    S3(#[from] S3Error),
}

impl From<S3ClientError> for ApiError {
    fn from(err: S3ClientError) -> Self {
        match err {
            S3ClientError::Serde(_) => ApiError::InternalServerError,
            S3ClientError::Payload(_) => ApiError::BadRequest,
            S3ClientError::HttpRequest(err) => ApiError::OtherServerResponseError(err.to_string()),
            S3ClientError::S3(err) => ApiError::OtherServerResponseError(err.to_string()),
        }
    }
}
