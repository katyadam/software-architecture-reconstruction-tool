use models::api::ProcessFilesIdentifier;
use serde::Deserialize;
use utoipa::{ToSchema, schema};

#[derive(Debug, ToSchema, Deserialize)]
pub struct PostViews {
    #[schema(
    example = json!({
        "codebase_uuid": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
        "commit_hash": "abc123def4567890abcdef1234567890abcdef12"
    })
)]
    pub identifier: ProcessFilesIdentifier,

    #[schema(
        example = "3fa85f64-5717-4562-b3fc-2c963f66afa6/e4109e71-9d0d-49ae-bb81-c7af1d07064a"
    )]
    pub base_dir_path: String,
}
