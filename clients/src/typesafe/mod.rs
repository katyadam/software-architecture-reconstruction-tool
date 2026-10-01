pub mod question;
pub mod response;
pub mod state;

use std::collections::BTreeMap;

use awc::Client;
use serde::Serialize;

use crate::{
    error::HttpClientError,
    http::client::HttpClient,
    typesafe::{question::Question, response::SystemOneResponse, state::ResidualCallState},
};

const SYSTEM_ONE_BASE_URL: &str = "https://api.typesafe.ai";

pub struct TypeSafeClient {
    client: HttpClient,
}

#[derive(Debug, Serialize)]
pub struct SystemOneRequest {
    state: ResidualCallState,
    model: String,
    questions: BTreeMap<String, Question>,
}

impl TypeSafeClient {
    pub fn new() -> Self {
        let api_key: String =
            std::env::var("TYPESAFE_API_KEY").expect("TYPE_SAFE_API_KEY to exist");
        let awc = Client::builder().bearer_auth(api_key).finish();
        let client = HttpClient::new(SYSTEM_ONE_BASE_URL.to_string(), awc);
        Self { client }
    }

    pub async fn system_one(
        &self,
        request: SystemOneRequest,
    ) -> Result<SystemOneResponse, HttpClientError> {
        let resp: SystemOneResponse = self.client.post_json("/v1/systemone", &request).await?;
        Ok(resp)
    }
}

impl Default for TypeSafeClient {
    fn default() -> Self {
        Self::new()
    }
}
