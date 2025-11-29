use clients::http::{client::HttpClient, error::HttpClientError};

use crate::connectors::dto::ConfigurationDto;

pub struct ManagerConnector {
    http_client: HttpClient,
}

impl ManagerConnector {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }

    pub async fn get_commit_configuration(
        &self,
        commit_hash: &str,
    ) -> Result<ConfigurationDto, HttpClientError> {
        self.http_client
            .get_json::<ConfigurationDto>(format!("/commits/{commit_hash}/conf").as_str())
            .await
    }

    pub async fn confirm_processed_commit(&self, commit_hash: &str) -> Result<(), HttpClientError> {
        self.http_client
            .patch(format!("/commits/{commit_hash}/processed").as_str())
            .await
    }
}
