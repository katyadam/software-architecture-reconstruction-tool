use log::info;
use models::CodeElementsAggregate;
use uuid::Uuid;

use crate::{api::dto::PostEntities, client::client::HttpClient, error::HttpClientError};

pub struct SynthesizerConnector {
    http_client: HttpClient,
}

impl SynthesizerConnector {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
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
