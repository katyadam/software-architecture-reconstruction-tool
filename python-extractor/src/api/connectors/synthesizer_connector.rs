use log::info;
use models::CodeElementsAggregate;
use uuid::Uuid;

use crate::{
    api::dto::PostEntities,
    client::{http::client::HttpClient, s3::client::S3Client},
    error::{HttpClientError, S3ClientError},
};

pub struct SynthesizerConnector {
    http_client: HttpClient,
    s3_client: S3Client,
}

impl SynthesizerConnector {
    pub fn new(http_client: HttpClient, s3_client: S3Client) -> Self {
        Self {
            http_client,
            s3_client,
        }
    }

    pub async fn store_code_elements(
        &self,
        code_elements: CodeElementsAggregate,
        codebase_uuid: Uuid,
        path: &str,
    ) -> Result<(), S3ClientError> {
        Ok(())
    }

    pub async fn send_code_elements(
        &self,
        code_elements: CodeElementsAggregate,
        codebase_uuid: Uuid,
    ) -> Result<(), HttpClientError> {
        let payload = PostEntities::new(codebase_uuid, code_elements.entities);
        self.http_client
            .post_json::<PostEntities, ()>("/context-maps", &payload)
            .await?;
        info!("Context Map Created - Entities: {:?}", payload.entities);
        Ok(())
    }
}
