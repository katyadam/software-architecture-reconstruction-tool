use serde::Serialize;

use crate::errors::api::ApiError;

// Errors related to conversion of Model Structs
#[derive(thiserror::Error, Debug, Serialize)]
pub enum ModelError {
    #[error("couldn't deserialize: {0}")]
    DeserializationError(String),
}

impl From<ModelError> for ApiError {
    fn from(err: ModelError) -> Self {
        match err {
            ModelError::DeserializationError(_) => ApiError::BadRequest,
        }
    }
}

impl From<serde_json::Error> for ModelError {
    fn from(err: serde_json::Error) -> Self {
        ModelError::DeserializationError(err.to_string())
    }
}
