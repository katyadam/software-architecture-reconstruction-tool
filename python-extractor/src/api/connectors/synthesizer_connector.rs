use clients::http::{client::HttpClient, error::HttpClientError};
use log::info;
use uuid::Uuid;

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
        codebase_uuid: Uuid,
        base_dir_path: &str,
    ) -> Result<(), HttpClientError> {
        self.http_client
            .post_json::<ViewsDto, ()>(
                "/views",
                &ViewsDto {
                    codebase_uuid,
                    base_dir_path,
                },
            )
            .await?;
        info!("Load info about {} sent to synthesizer.", base_dir_path);
        Ok(())
    }
}
