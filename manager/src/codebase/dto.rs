use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostCodebase {
    #[schema(example = "main")]
    pub branch: String,
    #[schema(example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    pub project_uuid: Uuid,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateCodebase {
    #[schema(example = "master")]
    pub new_branch: String,
    #[schema(example = "d4c3fd92-5941-4538-b93b-298fe22c99db")]
    pub new_owner: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CodebaseResponse {
    pub codebase_uuid: Uuid,
    pub branch: String,
    pub project_uuid: Uuid,
    pub created_at: String,
}
