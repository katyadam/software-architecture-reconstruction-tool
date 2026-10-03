//! Is a residual REST call an HTTP request, and what does its receiver talk to?
//! Internal vs external is left to resolution, which only matches configured services.

use std::collections::BTreeMap;

use super::{NoulCriteria, Question};

/// Answer key of the `is_http` Noul.
pub const IS_HTTP: &str = "is_http";
/// Answer key of the `receiver_kind` Choice.
pub const RECEIVER_KIND: &str = "receiver_kind";
/// [`RECEIVER_KIND`] option for a web service / REST API.
pub const HTTP_API: &str = "http_api";

pub fn residual_classification() -> BTreeMap<String, Question> {
    let kind = |desc: &str| Some(desc.to_string());
    BTreeMap::from([
        (
            IS_HTTP.to_string(),
            Question::Noul {
                instructions: "Does this call send an HTTP request over the network?".to_string(),
                criteria: Some(NoulCriteria {
                    yes: "HTTP client call (requests, httpx, aiohttp, RestTemplate, ...)"
                        .to_string(),
                    no: "Database/ORM, cache, dict or collection lookup, no network".to_string(),
                }),
            },
        ),
        (
            RECEIVER_KIND.to_string(),
            Question::Choice {
                instructions: "What does the receiver of this call talk to?".to_string(),
                criteria: BTreeMap::from([
                    (HTTP_API.to_string(), kind("A web service or REST API")),
                    (
                        "database".to_string(),
                        kind("A database, ORM or repository"),
                    ),
                    ("cache".to_string(), kind("A cache or key-value store")),
                    (
                        "in_memory_collection".to_string(),
                        kind("An in-process map, list or set"),
                    ),
                    ("container_runtime".to_string(), kind("A container engine")),
                    (
                        "message_broker".to_string(),
                        kind("A queue, pub/sub or message broker"),
                    ),
                    ("other".to_string(), None),
                ]),
            },
        ),
    ])
}
