use models::api::ProcessFilesIdentifier;
use serde::Serialize;
use uuid::Uuid;

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
pub struct ViewsDto<'a> {
    pub identifier: ProcessFilesIdentifier,
    pub base_dir_path: &'a str,
}
