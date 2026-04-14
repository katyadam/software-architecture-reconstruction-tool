use models::ir::evaluted::EvaluatedIR;

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
    pub async fn store_evaluated_ir(
        &self,
        ir: EvaluatedIR,
        path: &str,
    ) -> Result<(), S3ClientError> {
        self.s3_client.save_context_map(&ir.entities, path).await?;
        self.s3_client
            .save_sdg(&ir.endpoints, &ir.restcalls, path)
            .await?;
        self.s3_client
            .save_imcg(&ir.callables, &ir.call_statements, path)
            .await?;
        Ok(())
    }
}
