use futures_util::stream::{self, StreamExt};
use log::{debug, info, warn};
use models::{ConfigurationData, RestCall, ir::project::ProjectIR};
use sage::resolver::{
    client::SageClient,
    query::SageQuery,
    response::{SageError, SageResponse},
};

use crate::pipeline::pass3::llm_enhance::{
    baseline::log_baseline_buckets,
    matcher::{build_index, deterministic_match},
    query_builder::{build_query_for_restcall, rewrite_target_uri_to_service},
    residual_edge_filter::{ResidualTriage, triage},
    signals,
};

const MAX_CONCURRENT_LLM_QUERIES: usize = 4;

struct PendingQuery {
    index: usize,
    original_uri: String,
    query: SageQuery,
}

struct QueryOutcome {
    index: usize,
    original_uri: String,
    result: Result<SageResponse, SageError>,
}

pub async fn evaluate_restcalls_with_llm(
    restcalls: &mut [RestCall],
    config: &ConfigurationData,
    sage: &SageClient,
    project_ir: &ProjectIR,
) {
    // TEMPORARY (S0.3) measurement: emit the baseline bucket breakdown under the
    // current (old) lexical gate plus the S0.4 strong-vs-thin split over the
    // residual population. Runs before any network call, so the summary is
    // produced even when sage is unreachable. Remove once the Phase 1 gate
    // rework lands. (Replaces the per-line S0.1 `log_residual_signals` dump;
    // sample residual lines are still printed by the measurement itself.)
    log_baseline_buckets(restcalls, config, project_ir);

    // S1.7: report how many residuals the unified triage drops as non-edges
    // (intra-service DB/dict/route reads the lexical identifier swept in) before
    // they would have reached the LLM. Cheap second pass; signals are lazy.
    let excluded_non_edges = restcalls
        .iter()
        .filter(|rc| triage(rc, project_ir, config) == ResidualTriage::NonEdge)
        .count();
    info!(
        "residual edge filter: excluded {excluded_non_edges} non-edge residual(s) from resolution"
    );

    resolve_deterministically(restcalls, config, project_ir);

    let pending = collect_pending_queries(restcalls, config, project_ir);
    info!(
        "Number of REST calls to evaluate with LLM: {}",
        pending.len()
    );
    let outcomes = dispatch_queries_concurrently(pending, sage).await;
    apply_query_outcomes(restcalls, outcomes, config);
}

/// S2.3: deterministic identifier -> service resolution pass, run BEFORE the
/// LLM. For each cross-service residual the lexical matcher resolves
/// unambiguously, rewrite its `target_uri` onto the matched service's canonical
/// base in place. The rewritten uri then reads as `ResolvedURL`, so
/// `collect_pending_queries` naturally excludes it from the LLM batch. A matched
/// service with no configured URL leaves the uri unchanged -- the `!=` guard
/// treats that as an abstain (no-op).
fn resolve_deterministically(
    restcalls: &mut [RestCall],
    config: &ConfigurationData,
    project_ir: &ProjectIR,
) {
    let index = build_index(config);
    let mut resolved = 0usize;
    let mut candidates = 0usize;
    for rc in restcalls.iter_mut() {
        if triage(rc, project_ir, config) != ResidualTriage::NeedsResolution {
            continue;
        }
        candidates += 1;
        let sig = signals::extract(rc, project_ir, config);
        if let Some(service) = deterministic_match(&sig, &index) {
            let rewritten = rewrite_target_uri_to_service(&rc.target_uri, &service);
            if rewritten != rc.target_uri {
                rc.target_uri = rewritten;
                resolved += 1;
            }
        }
    }
    info!("deterministic resolver: resolved {resolved} of {candidates} cross-service residual(s)");
}

fn collect_pending_queries(
    restcalls: &[RestCall],
    config: &ConfigurationData,
    project_ir: &ProjectIR,
) -> Vec<PendingQuery> {
    restcalls
        .iter()
        .enumerate()
        .filter(|(_, rc)| triage(rc, project_ir, config) == ResidualTriage::NeedsResolution)
        .filter_map(|(index, rc)| {
            let query = build_query_for_restcall(rc, config, project_ir).or_else(|| {
                warn!(
                    "sage: skipping {} — no candidate services to classify",
                    rc.target_uri
                );
                None
            })?;
            Some(PendingQuery {
                index,
                original_uri: rc.target_uri.clone(),
                query,
            })
        })
        .collect()
}

async fn dispatch_queries_concurrently(
    pending: Vec<PendingQuery>,
    sage: &SageClient,
) -> Vec<QueryOutcome> {
    stream::iter(pending)
        .map(|p| async move {
            let result = sage.query(p.query).await;
            QueryOutcome {
                index: p.index,
                original_uri: p.original_uri,
                result,
            }
        })
        .buffer_unordered(MAX_CONCURRENT_LLM_QUERIES)
        .collect()
        .await
}

/// Apply the LLM's closed-set choices. A chosen service is rewritten onto its
/// canonical config URL via `rewrite_target_uri_to_service`; an abstain (`None`)
/// or an error leaves the residual untouched.
fn apply_query_outcomes(
    restcalls: &mut [RestCall],
    outcomes: Vec<QueryOutcome>,
    config: &ConfigurationData,
) {
    for outcome in outcomes {
        match outcome.result {
            Ok(resp) => match resp.service {
                Some(name) => match config.service_descriptions.iter().find(|d| d.name == name) {
                    Some(service) => {
                        restcalls[outcome.index].target_uri =
                            rewrite_target_uri_to_service(&outcome.original_uri, service);
                    }
                    None => {
                        warn!(
                            "sage: chosen service {name} not in config for {}",
                            outcome.original_uri
                        );
                    }
                },
                None => {
                    debug!("sage: abstained on {}", outcome.original_uri);
                }
            },
            Err(e) => {
                warn!("sage: query for {} failed: {e}", outcome.original_uri);
            }
        }
    }
}
