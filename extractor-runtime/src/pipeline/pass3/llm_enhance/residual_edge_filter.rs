//! Splits residuals into cross-service calls and non-edges (DB/dict `.get`
//! swept in by lexical REST-call identification).

use crate::pipeline::pass3::llm_enhance::signals::{self, CallSiteSignals};
use crate::pipeline::pass3::restcalls::{EvalState, is_restcall_evaluated_enough};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) enum ResidualEdge {
    CrossService,
    NonEdge,
}

/// Cross-service if target has `http` or `/`, or an operand name hints a URL.
/// Matching operands against service names was tried and fails: domain nouns
/// are service names (`annotation_id` vs `annotation-service`).
pub(super) fn classify_residual(rc: &models::RestCall, signals: &CallSiteSignals) -> ResidualEdge {
    if rc.target_uri.contains("http") || rc.target_uri.contains('/') {
        return ResidualEdge::CrossService;
    }
    if signals
        .operand_identifiers
        .iter()
        .any(|id| name_hints_url(id))
    {
        return ResidualEdge::CrossService;
    }
    ResidualEdge::NonEdge
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) enum ResidualTriage {
    Resolved,
    Empty,
    NonEdge,
    NeedsResolution,
}

/// Gate, then edge filter. Signals only computed when needed.
pub(super) fn triage(
    rc: &models::RestCall,
    project_ir: &models::ir::project::ProjectIR,
    config: &models::ConfigurationData,
) -> ResidualTriage {
    match is_restcall_evaluated_enough(rc) {
        EvalState::ResolvedURL => ResidualTriage::Resolved,
        EvalState::Junk => ResidualTriage::Empty,
        EvalState::NeedsResolution => {
            let signals = signals::extract(rc, project_ir, config);
            match classify_residual(rc, &signals) {
                ResidualEdge::CrossService => ResidualTriage::NeedsResolution,
                ResidualEdge::NonEdge => ResidualTriage::NonEdge,
            }
        }
    }
}

// ponytail: substring match over-matches (`BASE` in `database`, `PORT` in `report`).
fn name_hints_url(name: &str) -> bool {
    let upper = name.to_uppercase();
    ["URL", "URI", "HOST", "ENDPOINT", "BASE", "PORT"]
        .iter()
        .any(|needle| upper.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::{RestCall, source_code::SourceSpan};

    fn signals(operands: &[&str]) -> CallSiteSignals {
        CallSiteSignals {
            origin_service: "caller-service".to_string(),
            client_class: None,
            imports: vec![],
            operand_identifiers: operands.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn restcall(target_uri: &str) -> RestCall {
        RestCall {
            function_name: "do_call".to_string(),
            function_hash: "h1".to_string(),
            call_arguments: vec![],
            http_method: Default::default(),
            target_uri: target_uri.to_string(),
            file_path: "/proj/caller/client.py".to_string(),
            source_span: SourceSpan::new(0, 0),
        }
    }

    #[test]
    fn mds_url_operand_is_cross_service() {
        let rc = restcall("self._mds_url + url");
        let s = signals(&["self", "_mds_url", "url"]);
        assert_eq!(classify_residual(&rc, &s), ResidualEdge::CrossService);
    }

    #[test]
    fn bare_path_params_are_non_edges() {
        for target in ["class_id", "annotation_id", "item_id", "collection_id"] {
            let rc = restcall(target);
            let s = signals(&[target]);
            assert_eq!(
                classify_residual(&rc, &s),
                ResidualEdge::NonEdge,
                "expected NonEdge for {target}"
            );
        }
    }

    #[test]
    fn http_literal_is_cross_service() {
        let rc = restcall("http://x/y");
        let s = signals(&[]);
        assert_eq!(classify_residual(&rc, &s), ResidualEdge::CrossService);
    }

    #[test]
    fn path_in_target_is_cross_service() {
        let rc = restcall("base + \"/cases\"");
        let s = signals(&["base"]);
        assert_eq!(classify_residual(&rc, &s), ResidualEdge::CrossService);
    }

    use models::{
        ConfigurationData,
        configuration::ServiceDescription,
        ir::project::{ClassHierarchy, ImportGraph, ProjectIR},
    };
    use std::collections::HashMap;

    fn triage_config() -> ConfigurationData {
        ConfigurationData {
            service_descriptions: vec![
                ServiceDescription {
                    name: "caller-service".to_string(),
                    base_dir_path: "/proj/caller".to_string(),
                    urls: vec![],
                },
                ServiceDescription {
                    name: "medical-data-service".to_string(),
                    base_dir_path: "/proj/mds".to_string(),
                    urls: vec![],
                },
            ],
        }
    }

    fn triage_pir() -> ProjectIR {
        ProjectIR {
            files: vec![],
            import_graph: ImportGraph {
                resolved_imports: HashMap::new(),
            },
            class_hierarchy: ClassHierarchy {
                parents: HashMap::new(),
                children: HashMap::new(),
            },
            constants: HashMap::new(),
            callable_map: HashMap::new(),
            callables_by_file_hash: HashMap::new(),
        }
    }

    #[test]
    fn triage_http_target_is_resolved() {
        let rc = restcall("http://medical-data-service:8000/x");
        assert_eq!(
            triage(&rc, &triage_pir(), &triage_config()),
            ResidualTriage::Resolved
        );
    }

    #[test]
    fn triage_empty_target_is_empty() {
        let rc = restcall("");
        assert_eq!(
            triage(&rc, &triage_pir(), &triage_config()),
            ResidualTriage::Empty
        );
    }

    #[test]
    fn triage_url_hint_operand_needs_resolution() {
        let rc = restcall("self._mds_url + url");
        assert_eq!(
            triage(&rc, &triage_pir(), &triage_config()),
            ResidualTriage::NeedsResolution
        );
    }

    #[test]
    fn triage_bare_id_param_is_non_edge() {
        let rc = restcall("class_id");
        assert_eq!(
            triage(&rc, &triage_pir(), &triage_config()),
            ResidualTriage::NonEdge
        );
    }
}
