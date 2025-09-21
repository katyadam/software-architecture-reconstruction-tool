use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostFileRecord {
    #[schema(example = "d4c3fd92-5941-4538-b93b-298fe22c99db")]
    pub codebase_uuid: Uuid,
    #[schema(example = "/some/file")]
    pub file_path: String,
    #[schema(example = 4096)]
    pub file_size: i64,
}
