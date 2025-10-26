use std::any::TypeId;

use awc::Client;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::http::error::HttpClientError;

pub struct HttpClient {
    base_url: String,
    client: Client,
}

impl HttpClient {
    pub fn new(base_url: String, client: Client) -> Self {
        Self { base_url, client }
    }

    pub async fn post_json<T: Serialize, R: DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &T,
    ) -> Result<R, HttpClientError>
    where
        T: Serialize + ?Sized,
        R: DeserializeOwned + 'static,
    {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), endpoint);
        let body_json = serde_json::to_string(body)?;

        let mut response = self
            .client
            .post(url)
            .insert_header(("Content-Type", "application/json"))
            .send_body(body_json)
            .await?;

        let bytes = response.body().await?;

        // If the caller specifies the return type as unit, then always return unit,
        // regardless of what the underlying request actually returns.”
        if TypeId::of::<R>() == TypeId::of::<()>() {
            let unit: R = serde_json::from_value(Value::Null).map_err(HttpClientError::Serde)?;
            return Ok(unit);
        }

        // Check if the body is empty
        if bytes.is_empty() {
            // We can only successfully return an empty value `()`
            // if the caller has specified R to be the unit type.
            // This is a common pattern for APIs returning 204 No Content.
            // The `serde_json::from_slice` would fail, so we handle it here.
            let empty_result: Result<R, _> = serde_json::from_value(serde_json::Value::Null);
            return empty_result.map_err(HttpClientError::Serde);
        }

        // If the body is not empty, proceed with normal deserialization
        let resp_json: R = serde_json::from_slice(&bytes)?;

        Ok(resp_json)
    }

    #[allow(dead_code)]
    pub async fn get_json<R: DeserializeOwned>(&self, endpoint: &str) -> Result<R, HttpClientError>
    where
        R: DeserializeOwned + 'static,
    {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), endpoint);

        let mut response = self
            .client
            .get(url)
            .insert_header(("Accept", "application/json"))
            .send()
            .await?;

        let bytes = response.body().await?;

        if TypeId::of::<R>() == TypeId::of::<()>() {
            let unit: R = serde_json::from_value(Value::Null).map_err(HttpClientError::Serde)?;
            return Ok(unit);
        }

        if bytes.is_empty() {
            let empty_result: Result<R, _> = serde_json::from_value(serde_json::Value::Null);
            return empty_result.map_err(HttpClientError::Serde);
        }

        let resp_json: R = serde_json::from_slice(&bytes)?;
        Ok(resp_json)
    }
}
