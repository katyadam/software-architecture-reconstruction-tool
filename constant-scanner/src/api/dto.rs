use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConstantResponse {
    pub constant_uuid: Uuid,
    pub name: String,
    pub value: String,
    pub commit_hash: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConstantBatchInput {
    pub commit_hash: String,
    pub constants: Vec<ConstantItem>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConstantItem {
    pub name: String,
    pub value: String,
}
