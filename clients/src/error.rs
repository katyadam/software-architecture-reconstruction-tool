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
