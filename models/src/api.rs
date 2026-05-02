use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{CallStatement, Callable, Endpoint, Entity, Import, RestCall};

#[derive(Debug, Default)]
pub struct CodeElementsAggregate {
    pub imports: Vec<Import>,
    pub entities: Vec<Entity>,
    pub endpoints: Vec<Endpoint>,
    pub restcalls: Vec<RestCall>,
    pub callables: Vec<Callable>,
    pub call_statements: Vec<CallStatement>,
}

impl CodeElementsAggregate {
    pub fn new(
        imports: Vec<Import>,
        entities: Vec<Entity>,
        endpoints: Vec<Endpoint>,
        restcalls: Vec<RestCall>,
        callables: Vec<Callable>,
        call_statements: Vec<CallStatement>,
    ) -> Self {
        Self {
            imports,
            entities,
            endpoints,
            restcalls,
            callables,
            call_statements,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ProcessFilesIdentifier {
    pub codebase_uuid: Uuid,
    pub commit_hash: String,
}

#[derive(Debug, Error)]
pub enum ExtractionError {
    #[error("Deserialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Process ended with error: {0}")]
    Process(String),

    #[error("Extractor for file: {0} not found")]
    ExtractorNotFound(String),

    #[error("Error occured during symbolic evaluation: {0}")]
    SymbolicEvaluation(String),
}

/// OpenAPI schema stub for the multipart file upload body.
#[allow(dead_code)]
#[derive(ToSchema)]
pub struct MultipleFileUploadSchema {
    #[schema(value_type = [String], format = Binary)]
    pub files: Vec<String>,
}
