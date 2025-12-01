use clients::http::{client::HttpClient, error::HttpClientError};
use log::info;
use models::api::ProcessFilesIdentifier;

use crate::api::dto::ViewsDto;

pub struct SynthesizerConnector {
    http_client: HttpClient,
}

impl SynthesizerConnector {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }

    pub async fn send_load_info(
        &self,
        identifier: ProcessFilesIdentifier,
        base_dir_path: &str,
    ) -> Result<(), HttpClientError> {
        self.http_client
            .post_json::<ViewsDto, ()>(
                "/views",
                &ViewsDto {
                    identifier,
                    base_dir_path,
                },
            )
            .await?;
        info!("Load info about {base_dir_path} sent to synthesizer.");
        Ok(())
    }
}
