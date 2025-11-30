use models::CodeElementsAggregate;

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
        path: &str,
    ) -> Result<(), S3ClientError> {
        self.s3_client
            .save_context_map(&code_elements.entities, path)
            .await?;

        self.s3_client
            .save_sdg(&code_elements.endpoints, &code_elements.restcalls, path)
            .await?;

        self.s3_client
            .save_imcg(
                &code_elements.callables,
                &code_elements.call_statements,
                path,
            )
            .await?;
        Ok(())
    }
}
