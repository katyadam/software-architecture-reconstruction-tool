use thiserror::Error;

#[allow(dead_code)]
// Builder Layer Errors
#[derive(Error, Debug)]
pub enum BuilderError {
    #[error("builder error: {0}")]
    Error(String),
}
