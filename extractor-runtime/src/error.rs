use actix_web::{HttpResponse, ResponseError};
use awc::error::{PayloadError, SendRequestError};
use clients::http::error::HttpClientError;
use s3::error::S3Error;
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

#[allow(dead_code)]
#[derive(Error, Debug, Serialize, ToSchema)]
pub enum ApiError {
    #[error("internal server error: {0}")]
    InternalServerError(String),
    #[error("not found")]
    NotFound,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("sending http request resulted in error: {0}")]
    OtherServerResponseError(String),
    #[error("can't convert file to utf-8")]
    Utf8ConversionError,
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::InternalServerError(s) => HttpResponse::InternalServerError().json(s),
            ApiError::NotFound => HttpResponse::NotFound().json("not found"),
            ApiError::BadRequest(s) => HttpResponse::BadRequest().json(s),
            ApiError::OtherServerResponseError(msg) => HttpResponse::BadRequest().json(msg),
            ApiError::Utf8ConversionError => {
                HttpResponse::BadRequest().json("bad request - can't convert file to utf-8")
            }
        }
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            ApiError::InternalServerError(_) => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::NotFound => actix_web::http::StatusCode::NOT_FOUND,
            ApiError::BadRequest(_) => actix_web::http::StatusCode::BAD_REQUEST,
            ApiError::OtherServerResponseError(_) => actix_web::http::StatusCode::BAD_REQUEST,
            ApiError::Utf8ConversionError => actix_web::http::StatusCode::BAD_REQUEST,
        }
    }
}

impl From<HttpClientError> for ApiError {
    fn from(err: HttpClientError) -> Self {
        match err {
            HttpClientError::Serde(s) => ApiError::InternalServerError(s.to_string()),
            HttpClientError::HttpRequest(err) => {
                ApiError::OtherServerResponseError(err.to_string())
            }
            HttpClientError::Payload(s) => ApiError::BadRequest(s.to_string()),
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
            S3ClientError::Serde(s) => ApiError::InternalServerError(s.to_string()),
            S3ClientError::Payload(s) => ApiError::BadRequest(s.to_string()),
            S3ClientError::HttpRequest(err) => ApiError::OtherServerResponseError(err.to_string()),
            S3ClientError::S3(s) => ApiError::InternalServerError(s.to_string()),
        }
    }
}

#[derive(Debug, Error)]
pub enum ExtractionError {
    #[error("Deserialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Process ended with error: {0}")]
    Process(String),

    #[error("Extractor for file: {0} not found")]
    ExtractorNotFound(String),
}

impl From<ExtractionError> for ApiError {
    fn from(err: ExtractionError) -> Self {
        match err {
            ExtractionError::Serde(error) => ApiError::InternalServerError(error.to_string()),
            ExtractionError::Process(s) => ApiError::InternalServerError(s),
            ExtractionError::ExtractorNotFound(s) => ApiError::BadRequest(s),
        }
    }
}
