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
//! MEASUREMENT-ONLY (S1.5): nothing here changes what the matcher or dispatch
//! sees. Enforcement (dropping `NonEdge`s before resolution) is S1.7. See plan
//! §4 Phase 1.5.

use crate::pipeline::pass3::llm_enhance::signals::CallSiteSignals;

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
