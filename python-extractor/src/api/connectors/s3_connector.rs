use models::CodeElementsAggregate;
use uuid::Uuid;

use crate::{client::s3::client::S3Client, error::S3ClientError};

pub struct S3Connector {
    s3_client: S3Client,
}

impl S3Connector {
    pub fn new(s3_client: S3Client) -> Self {
        Self { s3_client }
    }
}

impl S3Connector {
    pub async fn store_code_elements(
        &self,
        code_elements: CodeElementsAggregate,
        codebase_uuid: Uuid,
        path: &str,
    ) -> Result<(), S3ClientError> {
        Ok(())
    }
}
