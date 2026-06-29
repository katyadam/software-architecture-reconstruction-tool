//! Residual edge hygiene (Phase 1.5).
//!
//! The structural gate ([`super::super::restcalls`]) marks every non-empty,
//! non-http residual as `NeedsResolution`. That set is polluted: REST-call
//! identification in `python-extractor` is purely lexical (any
//! `*.get/post/put/delete/patch(arg)`), so it sweeps in intra-service DB clients
//! (`ClassClient.get(class_id)`), route-internal reads
//! (`clients.annotation.get(annot_id)`), and dict `.get(bare_var)` -- none of
//! which are cross-service HTTP edges. This module classifies a residual as a
//! genuine cross-service call or a `NonEdge` by the URL-nature of its target.
//!
//! `classify_residual` is MEASUREMENT-ONLY (S1.5): it changes nothing the matcher
//! or dispatch sees; `baseline.rs` consumes it for its `EDGE HYGIENE` block.
//! Enforcement lives in [`triage`] (S1.7), which composes the gate with the edge
//! filter and is what `dispatch.rs` uses to DROP `NonEdge`s before resolution.
//! See plan §4 Phase 1.5.

use crate::pipeline::pass3::llm_enhance::signals::{self, CallSiteSignals};
use crate::pipeline::pass3::restcalls::{EvalState, is_restcall_evaluated_enough};

/// Whether a `NeedsResolution` residual is a genuine cross-service HTTP call or
/// a non-edge (intra-service DB/dict `.get`, route-internal read) that the
/// lexical REST-call identifier swept in. Kept DISTINCT from the gate's `Junk`
/// (empty) state so what we drop stays auditable. See plan §4 Phase 1.5.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) enum ResidualEdge {
    CrossService,
    NonEdge,
}

/// Classify a residual by target URL-nature. `CrossService` iff ANY of:
/// 1. `target_uri` contains `"http"`, OR
/// 2. `target_uri` contains `'/'` (a path is present), OR
/// 3. an operand identifier hints a URL (uppercased contains a host/url word).
///
/// Otherwise `NonEdge`.
///
/// Dropped signal (S1.6, 2026-06-29): a rule matching an operand token against
/// candidate service NAMES inverted the empaia split (127 cross / 25 non-edge,
/// the opposite of reality). Domain nouns ARE the service names -- a local
/// `annotation_id` shares `annotation` with `annotation-service`, and `class_id`
/// shares `id` with `id-mapper-service` -- so there is no lexical way to tell a
/// local id-param read from a call to the same-named service. Target URL-nature
/// (rules 1-3) is the discriminator that survives.
///
/// Known coarseness (acceptable, measurement-first): the rule-3 substring list
/// over-matches -- `BASE` hits `database`, `PORT` hits `report`. None of
/// empaia's observed non-edge operands (`class_id`, `annotation_id`, `item_id`,
/// `collection_id`) trip these, so it is not corrected here.
pub(super) fn classify_residual(
    rc: &models::RestCall,
    signals: &CallSiteSignals,
) -> ResidualEdge {
    // Rules 1 & 2: target itself is a URL/path expression.
    if rc.target_uri.contains("http") || rc.target_uri.contains('/') {
        return ResidualEdge::CrossService;
    }
    // Rule 3: an operand name hints a URL/host.
    if signals
        .operand_identifiers
        .iter()
        .any(|id| name_hints_url(id))
    {
        return ResidualEdge::CrossService;
    }
    ResidualEdge::NonEdge
}

/// The single residual decision: gate (`restcalls::is_restcall_evaluated_enough`)
/// composed with the edge filter ([`classify_residual`]). `Resolved`/`Empty` come
/// from the gate; a gate `NeedsResolution` is refined into `NeedsResolution`
/// (genuine cross-service residual -> forward) vs `NonEdge` (DB/dict/route noise
/// -> drop). `Empty` and `NonEdge` are kept DISTINCT so the two drop reasons stay
/// auditable. See plan §4 Phase 1.5.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) enum ResidualTriage {
    Resolved,
    Empty,
    NonEdge,
    NeedsResolution,
}

/// Triage a residual. Signals are computed LAZILY -- only on a gate
/// `NeedsResolution` -- so the bulk `ResolvedURL`/`Junk` calls never pay for
/// `signals::extract`.
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

/// Mirrors `ranking.rs::name_hints_url` (private there; replicated so this
/// module does not depend on `ranking`, which is deleted in Phase 3).
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

    fn signals(operands: &[&str], candidate_services: &[&str]) -> CallSiteSignals {
        CallSiteSignals {
            origin_service: "caller-service".to_string(),
            client_class: None,
            imports: vec![],
            operand_identifiers: operands.iter().map(|s| s.to_string()).collect(),
            candidate_services: candidate_services.iter().map(|s| s.to_string()).collect(),
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
        // `self._mds_url + url` -> url-hint on operand.
        let rc = restcall("self._mds_url + url");
        let s = signals(&["self", "_mds_url", "url"], &["medical-data-service"]);
        assert_eq!(
            classify_residual(&rc, &s),
            ResidualEdge::CrossService
        );
    }

    #[test]
    fn bare_path_params_are_non_edges() {
        for target in ["class_id", "annotation_id", "item_id", "collection_id"] {
            let rc = restcall(target);
            let s = signals(&[target], &["app-service", "billing-service"]);
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
        let s = signals(&[], &[]);
        assert_eq!(
            classify_residual(&rc, &s),
            ResidualEdge::CrossService
        );
    }

    #[test]
    fn path_in_target_is_cross_service() {
        // `base + "/cases"` contains a path separator.
        let rc = restcall("base + \"/cases\"");
        let s = signals(&["base"], &[]);
        assert_eq!(
            classify_residual(&rc, &s),
            ResidualEdge::CrossService
        );
    }

    // --- triage (S1.7): gate composed with edge filter ---

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

    // Empty `files` is fine: triage's operands come from `target_uri`, not the IR.
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
        // `self._mds_url + url` -> gate NeedsResolution, edge filter CrossService.
        let rc = restcall("self._mds_url + url");
        assert_eq!(
            triage(&rc, &triage_pir(), &triage_config()),
            ResidualTriage::NeedsResolution
        );
    }

    #[test]
    fn triage_bare_id_param_is_non_edge() {
        // `class_id` -> gate NeedsResolution, edge filter NonEdge -> dropped.
        let rc = restcall("class_id");
        assert_eq!(
            triage(&rc, &triage_pir(), &triage_config()),
            ResidualTriage::NonEdge
        );
    }

    #[test]
    fn id_param_sharing_a_service_token_is_still_non_edge() {
        // Regression for the dropped rule 4: `class_id` shares `id` with
        // `id-mapper-service` and `annotation_id` shares `annotation` with
        // `annotation-service`, yet both are local id-param reads, not edges.
        for target in ["class_id", "annotation_id"] {
            let rc = restcall(target);
            let s = signals(
                &[target],
                &["id-mapper-service", "annotation-service", "app-service"],
            );
            assert_eq!(
                classify_residual(&rc, &s),
                ResidualEdge::NonEdge,
                "expected NonEdge for {target}"
            );
        }
    }
}
