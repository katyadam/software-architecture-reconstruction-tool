use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostCommit {
    #[schema(example = "264636199f20e97e5b2770829d8944e652027a7b")]
    pub commit_hash: String,
    #[schema(example = "init project")]
    pub commit_message: String,
    #[schema(example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    pub codebase_uuid: Uuid,
    #[schema(example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    pub configuration_uuid: Uuid,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CommitResponse {
    pub commit_hash: String,
    pub commit_message: String,
    pub codebase_uuid: Uuid,
    pub configuration_uuid: Uuid,
    pub created_at: String,
    pub processed: bool,
}
