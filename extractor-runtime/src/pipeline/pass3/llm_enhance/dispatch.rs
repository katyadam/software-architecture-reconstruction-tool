use std::collections::HashMap;

use futures_util::stream::{self, StreamExt};
use log::{info, warn};
use models::{ConfigurationData, RestCall, assignments::VariableAddress, ir::project::ProjectIR};
use sage::resolver::{
    client::SageClient,
    query::SageQuery,
    response::{SageError, SageResponse},
};

use crate::pipeline::pass3::llm_enhance::{
    baseline::log_baseline_buckets,
    query_builder::{build_query_for_restcall, rewrite_target_uri_with_resolution},
    residual_edge_filter::{ResidualTriage, triage},
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
    restcalls: &mut Vec<RestCall>,
    variables: HashMap<VariableAddress, String>,
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

    let pending = collect_pending_queries(restcalls, &variables, config, sage, project_ir);
    info!(
        "Number of REST calls to evaluate with LLM: {}",
        pending.len()
    );
    let outcomes = dispatch_queries_concurrently(pending, sage).await;
    apply_query_outcomes(restcalls, outcomes);
}

fn collect_pending_queries(
    restcalls: &[RestCall],
    variables: &HashMap<VariableAddress, String>,
    config: &ConfigurationData,
    sage: &SageClient,
    project_ir: &ProjectIR,
) -> Vec<PendingQuery> {
    restcalls
        .iter()
        .enumerate()
        .filter(|(_, rc)| triage(rc, project_ir, config) == ResidualTriage::NeedsResolution)
        .filter_map(|(index, rc)| {
            let query = build_query_for_restcall(rc, variables, config, sage).or_else(|| {
                warn!(
                    "sage: skipping {} — cannot read {}",
                    rc.target_uri, rc.file_path
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

fn apply_query_outcomes(restcalls: &mut [RestCall], outcomes: Vec<QueryOutcome>) {
    for outcome in outcomes {
        match outcome.result {
            Ok(resp) => {
                if let Some(resolved) = resp.resolved {
                    restcalls[outcome.index].target_uri =
                        rewrite_target_uri_with_resolution(&outcome.original_uri, &resolved);
                }
            }
            Err(e) => {
                warn!("sage: query for {} failed: {e}", outcome.original_uri);
            }
        }
    }
}
