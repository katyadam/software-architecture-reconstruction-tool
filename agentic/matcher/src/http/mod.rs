//! HTTP adapter: REST call -> endpoint candidates by path segment unification.
//! Copied from `synthesizer/src/sdg/matching.rs`; free to diverge.

mod segments;

use std::collections::BTreeSet;

use agentic_models::{Address, InteractionKind};
use models::HttpMethod;
use models::interaction_kind::{InteractionSignals, classify, host_of, is_reflexive};
use strsim::levenshtein;

use crate::points::Point;
use segments::{has_evidence, is_var, literals, match_tag, segments};

/// How a candidate was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchTag {
    /// All segments literal and equal.
    Exact,
    /// Same length, at least one variable segment.
    Template,
    /// Matched on a trailing run of segments only.
    Suffix,
    /// Levenshtein fallback, no unified candidate.
    Fuzzy,
}

/// Endpoints `call` can reach, deterministic order (service name, uri).
pub fn candidates<'a>(
    call: &Point,
    target: &str,
    endpoints: &[&'a Point],
    all_urls: &[String],
) -> Vec<(&'a Point, MatchTag)> {
    let (scope, host_known) = scope(call, target, endpoints, all_urls);
    let c = segments(target);
    let leading_hole = !target.contains("://") && c.first().is_some_and(|s| is_var(s));

    let found = scope
        .iter()
        .filter_map(|e| {
            let s = segments(path(e));
            match_tag(&c, &s, leading_hole, host_known)
                .filter(|_| has_evidence(&c, &s))
                .map(|t| (*e, t, s))
        })
        .collect();

    let mut kept = drop_dominated(&c, drop_suffix_if_full(found));
    if kept.is_empty() {
        return fuzzy_fallback(target, &scope).into_iter().collect();
    }
    kept.sort_by(|(a, _), (b, _)| sort_key(a).cmp(&sort_key(b)));
    kept
}

/// Max Levenshtein distance, as a fraction of the longer uri, for a fuzzy match.
const DISSIMILARITY_PERCENT: f32 = 0.3;

/// Candidate with its tag and endpoint path segments.
type Found<'a, 's> = (&'a Point, MatchTag, Vec<&'s str>);

/// Same-method endpoints in the services `call` can reach; true when the host is known.
fn scope<'a>(
    call: &Point,
    target: &str,
    endpoints: &[&'a Point],
    all_urls: &[String],
) -> (Vec<&'a Point>, bool) {
    let own = &call.service;
    let host = host_of(target).to_ascii_lowercase();

    let same_method = || {
        endpoints
            .iter()
            .copied()
            .filter(|e| method(e) == method(call))
    };
    let owns_host = |e: &Point| {
        !host.is_empty()
            && e.service.name != own.name
            && e.service
                .urls
                .iter()
                .any(|u| host_of(u).eq_ignore_ascii_case(&host))
    };

    if is_reflexive(target, &own.urls, all_urls) {
        (
            same_method()
                .filter(|e| e.service.name == own.name)
                .collect(),
            true,
        )
    } else if same_method().any(owns_host) {
        (same_method().filter(|e| owns_host(e)).collect(), true)
    } else {
        (
            same_method()
                .filter(|e| e.service.name != own.name)
                .collect(),
            false,
        )
    }
}

/// Old Levenshtein argmin over `scope`, kept within `DISSIMILARITY_PERCENT`; ties -> smallest `sort_key`.
fn fuzzy_fallback<'a>(target: &str, scope: &[&'a Point]) -> Option<(&'a Point, MatchTag)> {
    scope
        .iter()
        .map(|e| (fuzzy_distance(e, target), *e))
        .min_by(|(da, a), (db, b)| da.cmp(db).then_with(|| sort_key(a).cmp(&sort_key(b))))
        .filter(|(d, e)| {
            (*d as f32) < path(e).len().max(target.len()) as f32 * DISSIMILARITY_PERCENT
        })
        .map(|(_, e)| (e, MatchTag::Fuzzy))
}

/// Smallest distance from `target` to the endpoint uri under each service url, or bare.
fn fuzzy_distance(e: &Point, target: &str) -> usize {
    let path = path(e).trim_start_matches('/');
    e.service
        .urls
        .iter()
        .map(String::as_str)
        .chain([""])
        .map(|u| levenshtein(&format!("{}/{path}", u.trim_end_matches('/')), target))
        .min()
        .unwrap_or(usize::MAX)
}

/// Drops Suffix matches when any Exact or Template match exists.
fn drop_suffix_if_full<'a, 's>(mut found: Vec<Found<'a, 's>>) -> Vec<Found<'a, 's>> {
    if found.iter().any(|(_, t, _)| *t != MatchTag::Suffix) {
        found.retain(|(_, t, _)| *t != MatchTag::Suffix);
    }
    found
}

/// Drops candidates whose literal matches are a strict subset of another's.
fn drop_dominated<'a>(c: &[&str], found: Vec<Found<'a, '_>>) -> Vec<(&'a Point, MatchTag)> {
    // ponytail: O(n^2) dominance scan, fine for per-call candidate counts.
    let lits: Vec<BTreeSet<usize>> = found.iter().map(|(_, _, s)| literals(c, s)).collect();
    found
        .iter()
        .zip(&lits)
        .filter(|(_, b)| !lits.iter().any(|a| a.is_superset(b) && a != *b))
        .map(|((e, t, _), _)| (*e, *t))
        .collect()
}

/// Deterministic ordering key for a candidate.
fn sort_key(e: &Point) -> (&str, &str, &str, &str) {
    (&e.service.name, path(e), e.function_name(), e.file())
}

fn method(p: &Point) -> Option<HttpMethod> {
    match &p.point.address {
        Address::Http { method, .. } => *method,
        _ => None,
    }
}

/// The address path: an endpoint's uri, a call's path without host.
fn path(p: &Point) -> &str {
    match &p.point.address {
        Address::Http { path, .. } => path,
        _ => "",
    }
}

/// Copy of synthesizer `request_kind`: classify by the matched endpoint's
/// path, falling back to the call target when there is none.
pub fn request_kind(
    call: &Point,
    target: &str,
    receiver: &Point,
    all_urls: &[String],
) -> InteractionKind {
    let endpoint_uri = path(receiver);
    let target_path = if endpoint_uri.is_empty() {
        target
    } else {
        endpoint_uri
    };
    classify(
        &InteractionSignals {
            caller_file: call.file(),
            target_path,
            target_uri: target,
        },
        &call.service.urls,
        all_urls,
    )
}

#[cfg(test)]
mod tests;
