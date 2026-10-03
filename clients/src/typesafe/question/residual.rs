//! Is a residual REST call a cross-service edge?

use std::collections::BTreeMap;

use super::{NoulCriteria, Question};

/// Answer keys of [`residual_classification`].
pub const IS_HTTP: &str = "is_http";
pub const IS_EXTERNAL: &str = "is_internal";

/// Residual REST call is an edge iff both answers are yes.
pub fn residual_classification() -> BTreeMap<String, Question> {
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
            IS_EXTERNAL.to_string(),
            Question::Noul {
                instructions: "Is the target another service of this system?".to_string(),
                criteria: Some(NoulCriteria {
                    yes: "Base URL from project config/settings/env naming an own service"
                        .to_string(),
                    no: "Third-party or public host, or no host at all".to_string(),
                }),
            },
        ),
    ])
}
