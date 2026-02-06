use clients::http::{client::HttpClient, error::HttpClientError};

use crate::connectors::dto::ConstantsDto;

pub struct ConstantScannerConnector {
    http_client: HttpClient,
}

impl ConstantScannerConnector {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }

    pub async fn get_commit_constants(
        &self,
        commit_hash: &str,
    ) -> Result<ConstantsDto, HttpClientError> {
        self.http_client
            .get_json::<ConstantsDto>(format!("/constants/{commit_hash}").as_str())
            .await
    }
}
