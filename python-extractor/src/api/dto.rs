use chrono::{DateTime, Utc};
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

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct ConfigurationDto {
    pub configuration_uuid: Uuid,
    pub project_uuid: Uuid,
    pub configuration_data: ConfigurationDataDto,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct ConfigurationDataDto {
    pub services: Vec<ServiceDto>,
}

#[derive(Deserialize)]
pub struct ServiceDto {
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct ViewsDto<'a> {
    pub codebase_uuid: Uuid,
    pub base_dir_path: &'a str,
}
