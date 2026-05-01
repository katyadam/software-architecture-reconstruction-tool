use std::collections::HashMap;

use models::ir::{ast::Expr, project::ConstantValue};
use strsim::levenshtein;

/// Convert project constants to the initial symbolic evaluation environment.
///
/// Quoted string values have their surrounding quotes stripped so that
/// `BASE_URL = "/api/v1"` becomes `Expr::Literal("/api/v1")`.
///
/// All constants are assigned dtype `"String"` regardless of their actual type.
/// Numeric and boolean constants are represented correctly as `Expr::Literal` values
/// but carry a `String` dtype, which is intentional for URI concat purposes.
pub(crate) fn constants_to_env(
    constants: &HashMap<String, ConstantValue>,
) -> HashMap<String, (Option<String>, Expr)> {
    constants
        .iter()
        .map(|(name, cv)| {
            let value = cv.value.trim_matches(|c| c == '"' || c == '\'').to_string();
            (
                name.clone(),
                (Some("String".to_string()), Expr::Literal(value)),
            )
        })
        .collect()
}

// Maximum normalized edit distance (dist / shorter_length) for the Levenshtein fallback.
const LEVENSHTEIN_THRESHOLD: f32 = 0.25;

/// Match a entity attribute name to a value in `external_constants`.
///
/// Two-phase lookup:
///
/// 1. **Suffix match** — prefixes often used for environment variables of any length.
///    `env_prefix = "mds_"` means `Settings.es_url` reads from `MDS_ES_URL`.
///    The suffix must cover at least half
///    the env var name to prevent short attrs (e.g. `url`) from greedily matching
///    long unrelated vars (e.g. `BASE_SERVICE_URL`).
///
/// 2. **Levenshtein fallback** — catches slight mismatches (alternate casing
///    schemes, minor typos). Distance is normalized by the shorter string length
///    so prefix length does not inflate the tolerance window.
pub(crate) fn derive_constant_value_by_external_constants(
    attr_name: &str,
    external_constants: &HashMap<String, String>,
) -> Option<String> {
    let attr = attr_name.to_ascii_lowercase();
    let suffix_candidate = format!("_{attr}");
    let mut best: Option<(&str, f32)> = None;

    for (env_name, env_value) in external_constants {
        let env = env_name.to_ascii_lowercase();

        if env == attr || env.ends_with(&suffix_candidate) {
            return Some(env_value.clone());
        }

        let min_len = attr.len().min(env.len()) as f32;
        if min_len == 0.0 {
            continue;
        }
        let normalized = levenshtein(&attr, &env) as f32 / min_len;
        if normalized < LEVENSHTEIN_THRESHOLD && best.map_or(true, |(_, prev)| normalized < prev) {
            best = Some((env_value, normalized));
        }
    }
    best.map(|(v, _)| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_constant_exact_suffix_short_prefix() {
        let mut external_constants = HashMap::new();
        external_constants.insert("MDS_AAA_URL".to_string(), "http://aaa.cz".to_string());
        assert_eq!(
            derive_constant_value_by_external_constants("aaa_url", &external_constants),
            Some("http://aaa.cz".to_string())
        );
    }

    #[test]
    fn test_derive_constant_exact_suffix_long_prefix() {
        let mut external_constants = HashMap::new();
        external_constants.insert(
            "SOME_LONG_PREFIX_AAA_URL".to_string(),
            "http://aaa.cz".to_string(),
        );
        assert_eq!(
            derive_constant_value_by_external_constants("aaa_url", &external_constants),
            Some("http://aaa.cz".to_string())
        );
    }

    #[test]
    fn test_derive_constant_exact_match_no_prefix() {
        let mut external_constants = HashMap::new();
        external_constants.insert("AAA_URL".to_string(), "http://aaa.cz".to_string());
        assert_eq!(
            derive_constant_value_by_external_constants("aaa_url", &external_constants),
            Some("http://aaa.cz".to_string())
        );
    }

    #[test]
    fn test_derive_constant_no_match() {
        let mut external_constants = HashMap::new();
        external_constants.insert("TOTALLY_UNRELATED".to_string(), "http://x.cz".to_string());
        assert_eq!(
            derive_constant_value_by_external_constants("aaa_url", &external_constants),
            None
        );
    }

    #[test]
    fn test_derive_constant_short_attr_matches_by_suffix() {
        // `url` matches `BASE_SERVICE_URL` via the `_url` word-boundary suffix.
        // If the pydantic env_prefix is `BASE_SERVICE_`, Settings.url -> BASE_SERVICE_URL.
        let mut external_constants = HashMap::new();
        external_constants.insert("BASE_SERVICE_URL".to_string(), "http://svc".to_string());
        assert_eq!(
            derive_constant_value_by_external_constants("url", &external_constants),
            Some("http://svc".to_string())
        );
    }
}
