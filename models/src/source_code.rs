use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Clone, Default)]
pub struct SourceSpan {
    pub start_byte: u32,
    pub end_byte: u32,
}

impl SourceSpan {
    pub fn new(start_byte: u32, end_byte: u32) -> Self {
        Self {
            start_byte,
            end_byte,
        }
    }
}
