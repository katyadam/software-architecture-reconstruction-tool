use std::collections::HashMap;

use futures_util::stream::{self, StreamExt};
use log::{info, warn};
use models::{ConfigurationData, RestCall, assignments::VariableAddress, ir::project::ProjectIR};
use sage::resolver::{
    client::SageClient,
    query::SageQuery,
    response::{SageError, SageResponse},
};

use crate::pipeline::pass3::{
    llm_enhance::query_builder::{build_query_for_restcall, rewrite_target_uri_with_resolution},
    llm_enhance::signals,
    restcalls::{EvalState, is_restcall_evaluated_enough},
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
    // TEMPORARY validation instrumentation (S0.1): emit the call-site signals for
    // every residual restcall before any network call, so the SIGNALS lines are
    // produced even when sage is unreachable. Remove/repurpose once the matcher
    // and LLM classifier consume these signals directly.
    log_residual_signals(restcalls, config, project_ir);

    let pending = collect_pending_queries(restcalls, &variables, config, sage);
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
) -> Vec<PendingQuery> {
    restcalls
        .iter()
        .enumerate()
        .filter(|(_, rc)| is_restcall_evaluated_enough(rc) == EvalState::NeedsLLM)
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

/// TEMPORARY (S0.1) validation hook: for every residual restcall (state is not
/// `Enough`, and the target_uri is non-empty), extract and log its call-site
/// signals. Does not alter dispatch behavior.
fn log_residual_signals(
    restcalls: &[RestCall],
    config: &ConfigurationData,
    project_ir: &ProjectIR,
) {
    for rc in restcalls {
        if rc.target_uri.is_empty()
            || is_restcall_evaluated_enough(rc) == EvalState::Enough
        {
            continue;
        }
        let s = signals::extract(rc, project_ir, config);
        info!(
            "SIGNALS: target_uri={} | file={} | origin_service={} | client_class={} | operand_identifiers=[{}] | imports=[{}] | candidate_services=[{}]",
            rc.target_uri,
            rc.file_path,
            s.origin_service,
            s.client_class.as_deref().unwrap_or("None"),
            s.operand_identifiers.join(", "),
            s.imports.join(", "),
            s.candidate_services.join(", "),
        );
    }
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
