use clients::{error::HttpClientError, http::client::HttpClient};

use crate::connectors::dto::Constant;

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
    ) -> Result<Vec<Constant>, HttpClientError> {
        self.http_client
            .get_json::<Vec<Constant>>(format!("/constants/{commit_hash}").as_str())
            .await
    }
}
