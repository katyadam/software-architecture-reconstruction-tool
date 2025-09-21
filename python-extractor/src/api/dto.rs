use models::Entity;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[allow(dead_code)]
#[derive(utoipa::ToSchema)]
pub struct MultipleFileUploadSchema {
    #[schema(value_type = [String], format = Binary)]
    pub files: Vec<String>,
}

#[derive(Serialize)]
pub struct PostFileRecord {
    pub codebase_uuid: Uuid,
    pub file_path: String,
    pub file_size: i64,
}

impl PostFileRecord {
    pub fn new(codebase_uuid: Uuid, file_path: String, file_size: i64) -> Self {
        Self {
            codebase_uuid,
            file_path,
            file_size,
        }
    }
}

#[derive(Serialize)]
pub struct PostEntities {
    pub codebase_uuid: Uuid,
    pub entities: Vec<Entity>,
}

impl PostEntities {
    pub fn new(codebase_uuid: Uuid, entities: Vec<Entity>) -> Self {
        Self {
            codebase_uuid,
            entities,
        }
    }
}

#[derive(Deserialize)]
pub struct ServiceNameQuery {
    pub service_name: String,
}
