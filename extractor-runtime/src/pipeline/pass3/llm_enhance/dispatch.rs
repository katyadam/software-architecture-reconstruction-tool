use futures_util::stream::{self, StreamExt};
use log::{debug, info, warn};
use models::{ConfigurationData, RestCall, ir::project::ProjectIR};
use sage::resolver::{
    client::SageClient,
    query::SageQuery,
    response::{SageError, SageResponse},
};

use crate::pipeline::pass3::llm_enhance::{
    matcher::{build_index, deterministic_match},
    oracle::{ServiceOracle, service_for_url},
    query_builder::{build_query_for_restcall, rewrite_target_uri_to_service},
    residual_edge_filter::{ResidualTriage, triage},
    scorer::{ProducedEdge, score},
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
    let excluded_non_edges = restcalls
        .iter()
        .filter(|rc| triage(rc, project_ir, config) == ResidualTriage::NonEdge)
        .count();
    info!(
        "residual edge filter: excluded {excluded_non_edges} non-edge residual(s) from resolution"
    );

    // Snapshot operands before rewrites change `target_uri`; used for scoring.
    let scored_residuals: Vec<(usize, Vec<String>)> = restcalls
        .iter()
        .enumerate()
        .filter(|(_, rc)| triage(rc, project_ir, config) == ResidualTriage::NeedsResolution)
        .map(|(i, rc)| {
            (
                i,
                signals::extract(rc, project_ir, config).operand_identifiers,
            )
        })
        .collect();

    resolve_deterministically(restcalls, config, project_ir);

    let pending = collect_pending_queries(restcalls, config, project_ir);
    info!(
        "Number of REST calls to evaluate with LLM: {}",
        pending.len()
    );
    let outcomes = dispatch_queries_concurrently(pending, sage).await;
    apply_query_outcomes(restcalls, outcomes, config);

    score_run(&scored_residuals, restcalls, config);
}

/// Score against the oracle when `SAGE_SCORE` (constants file path) is set.
/// Never fails the run.
fn score_run(
    residuals: &[(usize, Vec<String>)],
    restcalls: &[RestCall],
    config: &ConfigurationData,
) {
    let Some(path) = std::env::var_os("SAGE_SCORE") else {
        return;
    };
    let oracle = match ServiceOracle::from_constants_file(&path, config) {
        Ok(oracle) => oracle,
        Err(e) => {
            warn!("scorer: failed to load oracle from {path:?}: {e:#}");
            return;
        }
    };

    let produced: Vec<ProducedEdge> = residuals
        .iter()
        .map(|(i, ids)| ProducedEdge {
            identifiers: ids.clone(),
            chosen_service: service_for_url(&restcalls[*i].target_uri, config),
        })
        .collect();

    let s = score(&produced, &oracle);
    info!(
        "scorer: precision {:.3} recall {:.3} | correct {}/{} produced, {} scoreable | oracle {} edges ({} dropped)",
        s.precision,
        s.recall,
        s.correct,
        s.produced,
        s.scoreable,
        oracle.len(),
        oracle.dropped()
    );
}

/// Rewrite residuals the lexical matcher resolves onto the service URL.
/// Rewritten ones become `Resolved` and skip the LLM.
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

/// Rewrite onto the chosen service URL; abstain or error leaves it untouched.
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
