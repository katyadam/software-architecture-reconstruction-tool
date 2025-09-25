use crate::{api::dto::PostFileRecord, client::client::HttpClient, error::HttpClientError};

pub struct ManagerConnector {
    http_client: HttpClient,
}

impl ManagerConnector {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }

    pub async fn send_file_record(
        &self,
        file_record: PostFileRecord,
    ) -> Result<(), HttpClientError> {
        self.http_client
            .post_json::<PostFileRecord, ()>("/file-records", &file_record)
            .await
    }
}
