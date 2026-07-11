use std::collections::HashMap;

use models::{
    ConfigurationData, RestCall, assignments::VariableAddress, configuration::ServiceDescription,
};
use sage::resolver::{
    client::SageClient,
    code::CodeSnippet,
    facts::FactBundle,
    query::{QueryKind, SageQuery},
};

use crate::pipeline::{
    pass1::decide_language,
    pass3::llm_enhance::{ranking::rank_and_cap, variables::microservice_for_file},
};

pub(super) fn build_query_for_restcall(
    rc: &RestCall,
    variables: &HashMap<VariableAddress, String>,
    config: &ConfigurationData,
    sage: &SageClient,
) -> Option<SageQuery> {
    let snippet = build_snippet(rc)?;
    let lookup_key = rc
        .target_uri
        .split('/')
        .next()
        .unwrap_or(&rc.target_uri)
        .to_string();
    let microservice = microservice_for_file(&rc.file_path, config);
    let pruned = rank_and_cap(
        variables,
        &snippet.code,
        snippet.language,
        &microservice,
        &rc.file_path,
        sage.variables_budget(),
    );
    let bundle = FactBundle {
        sites: vec![snippet],
    };
    Some(SageQuery {
        bundle,
        kind: QueryKind::ResolveLookup { lookup_key },
        variables_map: pruned,
    })
}

/// Best-effort path-literal suffix of a residual target URI expression.
///
/// Returns the first `/`-leading path fragment, cleaned of surrounding quote or
/// concat syntax; `""` if none. Heuristic char scan: from the first `/`, take up
/// to the next quote, whitespace, or `+`. Deliberately partial -- not a parser.
fn path_suffix(original_uri: &str) -> String {
    let start = match original_uri.find('/') {
        Some(i) => i,
        None => return String::new(),
    };
    let mut out = String::new();
    for c in original_uri[start..].chars() {
        if c == '"' || c == '\'' || c.is_whitespace() || c == '+' {
            break;
        }
        out.push(c);
    }
    out.trim_end_matches(['"', '\'']).to_string()
}

/// Splice the residual's path suffix onto a resolved `base`, normalising the
/// join so the base's trailing slash never doubles the suffix's leading one.
pub(super) fn rewrite_onto_base(original_uri: &str, base: &str) -> String {
    format!(
        "{}{}",
        base.trim_end_matches('/'),
        path_suffix(original_uri)
    )
}

/// Rewrite a residual target URI onto a deterministically matched service's
/// canonical base URL. If the service carries no URL, returns `original_uri`
/// unchanged so the caller can treat it as an abstain (no resolution applied).
pub(super) fn rewrite_target_uri_to_service(
    original_uri: &str,
    service: &ServiceDescription,
) -> String {
    match service.urls.first() {
        Some(base) => rewrite_onto_base(original_uri, base),
        None => original_uri.to_string(),
    }
}

fn build_snippet(rc: &RestCall) -> Option<CodeSnippet> {
    let bytes = std::fs::read(&rc.file_path).ok()?;
    let start = rc.source_span.start_byte as usize;
    let end = rc.source_span.end_byte as usize;
    let slice = bytes.get(start..end)?;
    let code = String::from_utf8_lossy(slice).into_owned();
    let language = decide_language(&rc.file_path);
    Some(CodeSnippet { code, language })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_suffix_no_literal() {
        assert_eq!(path_suffix("self._mds_url + url"), "");
    }

    #[test]
    fn path_suffix_trailing_slash_literal() {
        assert_eq!(path_suffix("self._mds_url + \"/v1/cases/\""), "/v1/cases/");
    }

    #[test]
    fn path_suffix_stops_at_concat() {
        assert_eq!(path_suffix("base + \"/cases\" + case_id"), "/cases");
    }

    #[test]
    fn path_suffix_bare_literal() {
        assert_eq!(path_suffix("\"/annotations\""), "/annotations");
    }

    #[test]
    fn rewrite_to_service_with_url() {
        let service = ServiceDescription {
            name: "svc".to_string(),
            base_dir_path: "/proj/svc".to_string(),
            urls: vec!["http://svc:8000".to_string()],
        };
        assert_eq!(
            rewrite_target_uri_to_service("base + \"/cases\"", &service),
            "http://svc:8000/cases"
        );
    }

    #[test]
    fn rewrite_to_service_no_url_is_unchanged() {
        let service = ServiceDescription {
            name: "svc".to_string(),
            base_dir_path: "/proj/svc".to_string(),
            urls: vec![],
        };
        let original = "base + \"/cases\"";
        assert_eq!(rewrite_target_uri_to_service(original, &service), original);
    }
}
