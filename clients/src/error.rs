use awc::error::{PayloadError, SendRequestError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HttpClientError {
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("HTTP request error: {0}")]
    HttpRequest(#[from] SendRequestError),

    #[error("Wrong Payload: {0}")]
    Payload(#[from] PayloadError),
}

#[derive(Debug, Error)]
pub enum TypeSafeError {
    #[error("TYPESAFE_API_KEY not set: {0}")]
    MissingApiKey(#[from] std::env::VarError),

    #[error("TypeSafe request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("TypeSafe returned {status}: {body}")]
    Status { status: u16, body: String },
}
