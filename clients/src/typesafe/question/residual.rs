//! Is a residual REST call a web service request?
//! Internal vs external is left to resolution, which only matches configured services.

use std::collections::BTreeMap;

use super::{NoulCriteria, Question};

/// Answer key of the `is_http` Noul.
pub const IS_HTTP: &str = "is_http";

pub fn residual_classification() -> BTreeMap<String, Question> {
    BTreeMap::from([(
        IS_HTTP.to_string(),
        Question::Noul {
            instructions: "Does this call send a request to a web service or REST API endpoint?"
                .to_string(),
            criteria: Some(NoulCriteria {
                yes: "HTTP client call to a web service, or a wrapper around one. \
                          Python: requests, httpx, aiohttp (ClientSession, session.get), urllib3. \
                          Java: RestTemplate, WebClient, Feign, OkHttp, java.net.http.HttpClient. \
                          Go: net/http, resty."
                    .to_string(),
                no: "Not a web service call. Database driver or ORM (asyncpg, SQLAlchemy, \
                         psycopg, JDBC, JPA, gorm), cache or key-value store (redis, memcached), \
                         container engine SDK (docker, aiodocker), message broker client \
                         (kafka, pika), dict/map/collection lookup."
                    .to_string(),
            }),
        },
    )])
}
