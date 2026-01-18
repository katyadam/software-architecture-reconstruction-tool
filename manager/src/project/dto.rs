use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostProject {
    #[schema(example = "linux")]
    pub name: String,
    #[schema(example = "torvalds")]
    pub owner: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct UpdateProject {
    #[schema(example = "windows")]
    pub new_name: String,
    #[schema(example = "gates")]
    pub new_owner: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ProjectResponse {
    pub project_uuid: Uuid,
    pub name: String,
    pub owner: String,
    pub created_at: String,
}
