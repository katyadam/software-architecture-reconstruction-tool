use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{HttpMethod, Parameter};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema, Clone, Default)]
pub struct Endpoint {
    pub function_name: String,
    pub function_hash: String,
    pub http_method: HttpMethod,
    pub parameters: Vec<Parameter>,
    pub uri: String,
    pub file_path: String,
    pub router_variable: Option<String>,
    /// Function containing the route registration, `None` at module level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosing_function: Option<String>,
    /// Unevaluated prefix chains, outermost first: pass 2 -> pass 3 only, never serialized.
    #[serde(skip)]
    pub route_prefixes: Vec<Vec<PrefixExpr>>,
}

/// Router prefix expression and the file where it is written.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct PrefixExpr {
    pub file_path: String,
    pub expr: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_prefixes_not_serialized() {
        let endpoint = Endpoint {
            uri: "/x".to_string(),
            route_prefixes: vec![vec![PrefixExpr {
                file_path: "a.py".to_string(),
                expr: "BASE".to_string(),
            }]],
            ..Default::default()
        };
        let json = serde_json::to_value(&endpoint).expect("endpoint serializes");
        assert!(json.get("route_prefixes").is_none());
        let back: Endpoint = serde_json::from_value(json).expect("endpoint deserializes");
        assert!(back.route_prefixes.is_empty());
        assert_eq!(back.uri, "/x");
    }
}
