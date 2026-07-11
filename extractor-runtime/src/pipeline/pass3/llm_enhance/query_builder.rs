use models::{
    ConfigurationData, RestCall, configuration::ServiceDescription, ir::project::ProjectIR,
};
use sage::resolver::query::{CandidateService, ClassifyContext, QueryKind, SageQuery};

use crate::pipeline::pass3::llm_enhance::signals;

/// Build a closed-set classification query for a residual REST call.
///
/// Candidates are the configured services minus (a) the origin service (a call
/// site is never a self-loop) and (b) any service carrying no URL (e.g. `models`
/// -- not a runtime target). Returns `None` only when no candidate remains,
/// leaving nothing for the LLM to classify.
pub(super) fn build_query_for_restcall(
    rc: &RestCall,
    config: &ConfigurationData,
    project_ir: &ProjectIR,
) -> Option<SageQuery> {
    let sig = signals::extract(rc, project_ir, config);

    let candidates: Vec<CandidateService> = config
        .service_descriptions
        .iter()
        .filter(|desc| desc.name != sig.origin_service && !desc.urls.is_empty())
        .map(|desc| CandidateService {
            name: desc.name.clone(),
            url: desc.urls[0].clone(),
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    let context = ClassifyContext {
        origin_service: sig.origin_service,
        client_class: sig.client_class,
        imports: sig.imports,
        expression: rc.target_uri.clone(),
        operand_identifiers: sig.operand_identifiers,
    };

    Some(SageQuery {
        kind: QueryKind::ClassifyTargetService { candidates },
        context,
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
