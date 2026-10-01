pub mod question;
pub mod response;
pub mod state;

use std::{collections::BTreeMap, time::Duration};

use serde::Serialize;

use crate::{
    error::TypeSafeError,
    typesafe::{question::Question, response::SystemOneResponse, state::ResidualCallState},
};

const SYSTEM_ONE_URL: &str = "https://api.typesafe.ai/v1/systemone";
/// Pinned: `jev-latest` moves, which breaks run-to-run reproducibility.
pub const JEV_MODEL: &str = "jev-1.13.0";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Serialize)]
pub struct SystemOneRequest {
    pub state: ResidualCallState,
    pub model: &'static str,
    pub questions: BTreeMap<String, Question>,
}

impl SystemOneRequest {
    pub fn new(state: ResidualCallState, questions: BTreeMap<String, Question>) -> Self {
        Self {
            state,
            model: JEV_MODEL,
            questions,
        }
    }
}

/// No `Debug`: it would print the API key.
pub struct TypeSafeClient {
    http: reqwest::Client,
    api_key: String,
}

impl TypeSafeClient {
    /// Reads the key from `TYPESAFE_API_KEY`.
    pub fn new() -> Result<Self, TypeSafeError> {
        let api_key = std::env::var("TYPESAFE_API_KEY")?;
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        Ok(Self { http, api_key })
    }

    // ponytail: no retry on 429/529; add backoff if rate limits bite.
    pub async fn system_one(
        &self,
        request: &SystemOneRequest,
    ) -> Result<SystemOneResponse, TypeSafeError> {
        let response = self
            .http
            .post(SYSTEM_ONE_URL)
            .bearer_auth(&self.api_key)
            .json(request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(TypeSafeError::Status {
                status: status.as_u16(),
                body,
            });
        }
        Ok(response.json().await?)
    }
}
