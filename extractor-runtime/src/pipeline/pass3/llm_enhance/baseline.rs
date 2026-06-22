//! TEMPORARY (S0.3) measurement instrumentation.
//!
//! This module is NOT a permanent feature. It quantifies the silent recall
//! hole created by the lexical gate `restcalls::is_restcall_evaluated_enough`
//! (GATE A) and sizes the strong-vs-thin signal split (S0.4) over the residual
//! population. It only observes -- it never changes the gate's behavior, the
//! dispatch path, or any restcall. Remove this whole module once the Phase 1
//! gate rework lands.

use log::info;
use models::{ConfigurationData, RestCall, ir::project::ProjectIR};

use crate::pipeline::pass3::llm_enhance::signals;
use crate::pipeline::pass3::restcalls::{EvalState, is_restcall_evaluated_enough};

/// Number of sample residual SIGNALS lines to print (kept small to bound log
/// noise now that the per-line dump is replaced by the aggregated summary).
const SAMPLE_RESIDUAL_LINES: usize = 8;

/// The four observation buckets, refining the old gate's three `EvalState`
/// variants by splitting `Junk` into empty vs non-empty target_uri.
#[derive(PartialEq, Eq, Clone, Copy)]
enum Bucket {
    /// `target_uri` starts with "http": resolved, skip LLM.
    Enough,
    /// Passes the url/uri lexical test: sent to resolver.
    NeedsLlm,
    /// `target_uri` is empty: nothing to resolve.
    JunkEmpty,
    /// Non-empty, non-http, fails the lexical test -- the silent recall hole.
    JunkNonEmpty,
}

/// Classify a single restcall into one of the four observation buckets. This
/// mirrors the old gate exactly; for the two cases the gate collapses into
/// `Junk`, it inspects `target_uri` to split empty from non-empty.
fn classify(rc: &RestCall) -> Bucket {
    match is_restcall_evaluated_enough(rc) {
        EvalState::Enough => Bucket::Enough,
        EvalState::NeedsLLM => Bucket::NeedsLlm,
        EvalState::Junk => {
            if rc.target_uri.is_empty() {
                Bucket::JunkEmpty
            } else {
                Bucket::JunkNonEmpty
            }
        }
    }
}

/// Lowercased token set drawn from a candidate service name (split on the usual
/// separators) -- used for the loose import/service token overlap check.
fn service_tokens(service_name: &str) -> Vec<String> {
    service_name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(|t| t.to_ascii_lowercase())
        .collect()
}

/// Loose structural check: does any import rendered string share a token with
/// some candidate service name?
fn import_matches_a_service(imports: &[String], candidate_services: &[String]) -> bool {
    let service_tokens: Vec<String> = candidate_services
        .iter()
        .flat_map(|s| service_tokens(s))
        .collect();
    if service_tokens.is_empty() {
        return false;
    }
    imports.iter().any(|imp| {
        let lower = imp.to_ascii_lowercase();
        service_tokens.iter().any(|tok| lower.contains(tok.as_str()))
    })
}

/// Emit the S0.3 baseline-buckets summary and the folded-in S0.4
/// strong-vs-thin split. Pure observation; mutates nothing.
pub(super) fn log_baseline_buckets(
    restcalls: &[RestCall],
    config: &ConfigurationData,
    project_ir: &ProjectIR,
) {
    let total = restcalls.len();
    let mut enough = 0usize;
    let mut needs_llm = 0usize;
    let mut junk_empty = 0usize;
    let mut junk_non_empty = 0usize;

    for rc in restcalls {
        match classify(rc) {
            Bucket::Enough => enough += 1,
            Bucket::NeedsLlm => needs_llm += 1,
            Bucket::JunkEmpty => junk_empty += 1,
            Bucket::JunkNonEmpty => junk_non_empty += 1,
        }
    }

    info!("BASELINE BUCKETS (old lexical gate):");
    info!("  total restcalls:        {total}");
    info!("  Enough:                 {enough}   ({})", pct(enough, total));
    info!(
        "  NeedsLLM:               {needs_llm}   ({})",
        pct(needs_llm, total)
    );
    info!(
        "  Junk:empty:             {junk_empty}   ({})",
        pct(junk_empty, total)
    );
    info!(
        "  Junk:non-empty:         {junk_non_empty}   ({})   <- silent recall hole",
        pct(junk_non_empty, total)
    );

    // Residual population = the calls a resolver *could* act on:
    // NeedsLLM + Junk:non-empty (everything that is NOT Enough and NOT empty).
    let residuals: Vec<&RestCall> = restcalls
        .iter()
        .filter(|rc| matches!(classify(rc), Bucket::NeedsLlm | Bucket::JunkNonEmpty))
        .collect();

    let residual_total = residuals.len();
    let mut strong = 0usize;
    let mut empty_hash = 0usize;
    let mut samples_printed = 0usize;

    for rc in &residuals {
        if rc.function_hash.is_empty() {
            empty_hash += 1;
        }
        let s = signals::extract(rc, project_ir, config);
        let is_strong = s.client_class.is_some()
            || import_matches_a_service(&s.imports, &s.candidate_services);
        if is_strong {
            strong += 1;
        }

        if samples_printed < SAMPLE_RESIDUAL_LINES {
            info!(
                "SAMPLE SIGNALS [{}]: target_uri={} | file={} | origin_service={} | client_class={} | imports=[{}] | operand_identifiers=[{}]",
                if is_strong { "strong" } else { "thin" },
                rc.target_uri,
                rc.file_path,
                s.origin_service,
                s.client_class.as_deref().unwrap_or("None"),
                s.imports.join(", "),
                s.operand_identifiers.join(", "),
            );
            samples_printed += 1;
        }
    }
    let thin = residual_total - strong;

    info!("STRONG-VS-THIN (residual population = NeedsLLM + Junk:non-empty):");
    info!("  residual total:         {residual_total}");
    info!(
        "  strong (class/import):  {strong}   ({})",
        pct(strong, residual_total)
    );
    info!("  thin:                   {thin}   ({})", pct(thin, residual_total));
    info!(
        "  empty function_hash:    {empty_hash}   ({})   <- class unrecoverable regardless",
        pct(empty_hash, residual_total)
    );
}

/// Format `n` as a whole-number percentage of `total`, guarding against zero.
fn pct(n: usize, total: usize) -> String {
    match (n * 100).checked_div(total) {
        Some(p) => format!("{p}%"),
        None => "0%".to_string(),
    }
}
