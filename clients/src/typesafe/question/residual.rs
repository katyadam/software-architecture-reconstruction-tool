//! Is a residual REST call an HTTP request? Internal vs external is left to
//! resolution, which only matches configured services.

use std::collections::BTreeMap;

use super::{NoulCriteria, Question};

/// Answer key of [`residual_classification`].
pub const IS_HTTP: &str = "is_http";

pub fn residual_classification() -> BTreeMap<String, Question> {
    BTreeMap::from([(
        IS_HTTP.to_string(),
        Question::Noul {
            instructions: "Does this call send an HTTP request over the network?".to_string(),
            criteria: Some(NoulCriteria {
                yes: "HTTP client call (requests, httpx, aiohttp, RestTemplate, ...)".to_string(),
                no: "Database/ORM, cache, dict or collection lookup, no network".to_string(),
            }),
        },
    )])
}
