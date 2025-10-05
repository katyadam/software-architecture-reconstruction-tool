use log::info;

use crate::{client::http::client::HttpClient, error::HttpClientError};

pub struct SynthesizerConnector {
    http_client: HttpClient,
}

impl SynthesizerConnector {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }

    pub async fn send_load_info(&self, base_dir_path: &str) -> Result<(), HttpClientError> {
        self.http_client
            .post_json::<str, ()>("/load-info", base_dir_path)
            .await?;
        info!("Load info about {} sent to synthesizer.", base_dir_path);
        Ok(())
    }
}
