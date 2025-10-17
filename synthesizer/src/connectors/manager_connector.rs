use clients::http::{client::HttpClient, error::HttpClientError};
use uuid::Uuid;

use crate::connectors::dto::ConfigurationDto;

pub struct ManagerConnector {
    http_client: HttpClient,
}

impl ManagerConnector {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }

    pub async fn get_codebase_configuration(
        &self,
        codebase_uuid: Uuid,
    ) -> Result<ConfigurationDto, HttpClientError> {
        self.http_client
            .get_json::<ConfigurationDto>(format!("/codebases/{codebase_uuid}/conf").as_str())
            .await
    }
}
