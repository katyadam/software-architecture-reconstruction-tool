use std::str::FromStr;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Clone, Default)]
pub enum HttpMethod {
    #[default]
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
    HEAD,
    OPTIONS,
}

impl FromStr for HttpMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "GET" | "GET_STREAM_RESPONSE" => Ok(HttpMethod::GET),
            "POST" | "POST_STREAM_RESPONSE" => Ok(HttpMethod::POST),
            "PUT" | "PUT_STREAM_RESPONSE" => Ok(HttpMethod::PUT),
            "DELETE" | "DELETE_STREAM_RESPONSE" => Ok(HttpMethod::DELETE),
            "PATCH" | "PATCH_STREAM_RESPONSE" => Ok(HttpMethod::PATCH),
            "HEAD" | "HEAD_STREAM_RESPONSE" => Ok(HttpMethod::HEAD),
            "OPTIONS" | "OPTIONS_STREAM_RESPONSE" => Ok(HttpMethod::OPTIONS),
            _ => Err(format!("Invalid HTTP method: {s}")),
        }
    }
}
