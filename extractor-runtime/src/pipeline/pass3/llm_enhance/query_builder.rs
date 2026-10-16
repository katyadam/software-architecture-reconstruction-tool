use models::{
    ConfigurationData, RestCall, configuration::ServiceDescription, ir::project::ProjectIR,
};
use sage::resolver::query::{CandidateService, ClassifyContext, QueryKind, SageQuery};

use crate::pipeline::pass3::llm_enhance::signals;

/// Candidates: services with a URL, minus origin. `None` if none remain.
pub(super) fn build_query_for_restcall(
    rc: &RestCall,
    config: &ConfigurationData,
    project_ir: &ProjectIR,
) -> Option<SageQuery> {
    let sig = signals::extract(rc, project_ir, config);

    let candidates: Vec<CandidateService> = candidate_services(&sig.origin_service, config)
        .into_iter()
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

/// Services with a URL, minus `origin`.
pub(super) fn candidate_services<'a>(
    origin: &str,
    config: &'a ConfigurationData,
) -> Vec<&'a ServiceDescription> {
    config
        .service_descriptions
        .iter()
        .filter(|d| d.name != origin && !d.urls.is_empty())
        .collect()
}

/// `base + "/cases" + id` -> `/cases`. Heuristic scan, not a parser.
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

pub(super) fn rewrite_onto_base(original_uri: &str, base: &str) -> String {
    format!(
        "{}{}",
        base.trim_end_matches('/'),
        path_suffix(original_uri)
    )
}

/// No service URL -> `original_uri` unchanged (abstain).
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
    fn candidates_skip_origin_and_urlless() {
        let svc = |name: &str, url: Option<&str>| ServiceDescription {
            name: name.to_string(),
            base_dir_path: format!("/proj/{name}"),
            urls: url.map(str::to_string).into_iter().collect(),
        };
        let config = ConfigurationData {
            service_descriptions: vec![
                svc("caller", Some("http://caller")),
                svc("mds", Some("http://mds")),
                svc("nourl", None),
            ],
        };
        let names: Vec<&str> = candidate_services("caller", &config)
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(names, vec!["mds"]);
    }

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
