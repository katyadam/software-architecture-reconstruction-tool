use clients::typesafe::TypeSafeClient;
use futures_util::stream::{self, StreamExt};
use log::{debug, info, warn};
use models::{ConfigurationData, RestCall, ir::project::ProjectIR};
use sage::resolver::client::SageClient;

use crate::pipeline::pass3::llm_enhance::{
    matcher::{IndexedService, build_index, deterministic_match},
    oracle::{ServiceOracle, service_for_url},
    query_builder::{build_query_for_restcall, rewrite_target_uri_to_service},
    residual_edge_filter::{ResidualTriage, print_triage_footer, print_triage_header, triage},
    scorer::{ProducedEdge, score},
    signals,
};
use crate::pipeline::pass3::restcalls::OperandBindings;

const MAX_CONCURRENT_LLM_QUERIES: usize = 4;

struct Ctx<'a> {
    config: &'a ConfigurationData,
    project_ir: &'a ProjectIR,
    sage: &'a SageClient,
    typesafe_client: Option<&'a TypeSafeClient>,
    index: Vec<IndexedService<'a>>,
}

enum Outcome {
    /// Not a residual: resolved URL or empty target.
    Skipped,
    /// Jev: residual is not a cross-service call.
    NonEdge,
    /// `identifiers` are snapshotted before any rewrite; used for scoring.
    /// `jev_failed`: Jev errored, kept as residual.
    Residual {
        identifiers: Vec<String>,
        step: Step,
        jev_failed: bool,
    },
}

enum Step {
    Deterministic(String),
    Llm(String),
    Unresolved { queried: bool },
}

/// `bindings` is index-aligned with `restcalls`.
pub async fn evaluate_restcalls_with_llm(
    restcalls: &mut [RestCall],
    bindings: &[OperandBindings],
    config: &ConfigurationData,
    sage: &SageClient,
    typesafe_client: Option<&TypeSafeClient>,
    project_ir: &ProjectIR,
) {
    let ctx = Ctx {
        config,
        project_ir,
        sage,
        typesafe_client,
        index: build_index(config),
    };

    print_triage_header();
    let started = std::time::Instant::now();
    let outcomes: Vec<(usize, Outcome)> = stream::iter(restcalls.iter().enumerate())
        .map(|(i, rc)| {
            let ctx = &ctx;
            async move { (i, resolve_one(rc, &bindings[i], ctx).await) }
        })
        .buffer_unordered(MAX_CONCURRENT_LLM_QUERIES)
        .collect()
        .await;
    print_triage_footer(started.elapsed());

    let (mut residuals, mut deterministic, mut queried) = (0usize, 0usize, 0usize);
    let (mut non_edges, mut jev_errors) = (0usize, 0usize);
    let mut scored: Vec<(usize, Vec<String>)> = Vec::new();
    for (i, outcome) in outcomes {
        let (identifiers, step) = match outcome {
            Outcome::Skipped => continue,
            Outcome::NonEdge => {
                non_edges += 1;
                continue;
            }
            Outcome::Residual {
                identifiers,
                step,
                jev_failed,
            } => {
                jev_errors += usize::from(jev_failed);
                (identifiers, step)
            }
        };
        residuals += 1;
        match step {
            Step::Deterministic(uri) => {
                deterministic += 1;
                restcalls[i].target_uri = uri;
            }
            Step::Llm(uri) => {
                queried += 1;
                restcalls[i].target_uri = uri;
            }
            Step::Unresolved { queried: true } => queried += 1,
            Step::Unresolved { queried: false } => {}
        }
        scored.push((i, identifiers));
    }
    if typesafe_client.is_some() {
        // ponytail: errors fall back to residual; fail fast on 401/422 before evaluation runs.
        info!(
            "jev: {non_edges} non-edge(s), {jev_errors} error(s) of {} residual(s)",
            non_edges + residuals
        );
    }
    info!(
        "deterministic resolver: resolved {deterministic} of {residuals} cross-service residual(s)"
    );
    info!("Number of REST calls evaluated with LLM: {queried}");

    score_run(&scored, restcalls, config);
}

/// Triage -> deterministic matcher -> Sage.
async fn resolve_one(rc: &RestCall, bindings: &OperandBindings, ctx: &Ctx<'_>) -> Outcome {
    let (triaged, jev_failed) = match triage(
        rc,
        bindings,
        ctx.project_ir,
        ctx.config,
        ctx.typesafe_client,
    )
    .await
    {
        Ok(triaged) => (triaged, false),
        Err(e) => {
            warn!(
                "jev: classifying {} failed, keeping as residual: {e}",
                rc.target_uri
            );
            (ResidualTriage::NeedsResolution, true)
        }
    };
    match triaged {
        ResidualTriage::NeedsResolution => {}
        ResidualTriage::NonEdge => return Outcome::NonEdge,
        ResidualTriage::Resolved | ResidualTriage::Empty => return Outcome::Skipped,
    }

    let signals = signals::extract(rc, ctx.project_ir, ctx.config);
    let step = match deterministic_match(&signals, &ctx.index)
        .map(|service| rewrite_target_uri_to_service(&rc.target_uri, &service))
        .filter(|rewritten| *rewritten != rc.target_uri)
    {
        Some(uri) => Step::Deterministic(uri),
        None => query_sage(rc, ctx).await,
    };

    Outcome::Residual {
        identifiers: signals.operand_identifiers,
        step,
        jev_failed,
    }
}

/// Rewrite onto the chosen service URL; abstain or error leaves it untouched.
async fn query_sage(rc: &RestCall, ctx: &Ctx<'_>) -> Step {
    let Some(query) = build_query_for_restcall(rc, ctx.config, ctx.project_ir) else {
        warn!(
            "sage: skipping {} — no candidate services to classify",
            rc.target_uri
        );
        return Step::Unresolved { queried: false };
    };
    match ctx.sage.query(query).await {
        Ok(resp) => match resp.service {
            Some(name) => match ctx
                .config
                .service_descriptions
                .iter()
                .find(|d| d.name == name)
            {
                Some(service) => Step::Llm(rewrite_target_uri_to_service(&rc.target_uri, service)),
                None => {
                    warn!(
                        "sage: chosen service {name} not in config for {}",
                        rc.target_uri
                    );
                    Step::Unresolved { queried: true }
                }
            },
            None => {
                debug!("sage: abstained on {}", rc.target_uri);
                Step::Unresolved { queried: true }
            }
        },
        Err(e) => {
            warn!("sage: query for {} failed: {e}", rc.target_uri);
            Step::Unresolved { queried: true }
        }
    }
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
