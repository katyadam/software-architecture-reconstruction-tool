use std::str::FromStr;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Clone, Copy, Default)]
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

impl HttpMethod {
    pub const ALL: [Self; 7] = [
        Self::GET,
        Self::POST,
        Self::PUT,
        Self::DELETE,
        Self::PATCH,
        Self::HEAD,
        Self::OPTIONS,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::GET => "GET",
            Self::POST => "POST",
            Self::PUT => "PUT",
            Self::DELETE => "DELETE",
            Self::PATCH => "PATCH",
            Self::HEAD => "HEAD",
            Self::OPTIONS => "OPTIONS",
        }
    }
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
