//! REST call -> endpoint candidates by path segment unification.
//! Copied from `synthesizer/src/sdg/matching.rs`; free to diverge.

mod segments;

use std::collections::BTreeSet;

use models::interaction_kind::{host_of, is_reflexive};
use strsim::levenshtein;

use crate::points::Placed;
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
    call: &Placed,
    endpoints: &'a [Placed],
    all_urls: &[String],
) -> Vec<(&'a Placed, MatchTag)> {
    let (scope, host_known) = scope(call, endpoints, all_urls);
    let target = &call.target;
    let c = segments(target);
    let leading_hole = !target.contains("://") && c.first().is_some_and(|s| is_var(s));

    let found = scope
        .iter()
        .filter_map(|e| {
            let s = segments(&e.target);
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
type Found<'a, 's> = (&'a Placed, MatchTag, Vec<&'s str>);

/// Same-method endpoints in the services `call` can reach; true when the host is known.
fn scope<'a>(
    call: &Placed,
    endpoints: &'a [Placed],
    all_urls: &[String],
) -> (Vec<&'a Placed>, bool) {
    let target = &call.target;
    let own = &call.service;
    let host = host_of(target).to_ascii_lowercase();

    let same_method = || endpoints.iter().filter(|e| e.method == call.method);
    let owns_host = |e: &Placed| {
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
fn fuzzy_fallback<'a>(target: &str, scope: &[&'a Placed]) -> Option<(&'a Placed, MatchTag)> {
    scope
        .iter()
        .map(|e| (fuzzy_distance(e, target), *e))
        .min_by(|(da, a), (db, b)| da.cmp(db).then_with(|| sort_key(a).cmp(&sort_key(b))))
        .filter(|(d, e)| {
            (*d as f32) < e.target.len().max(target.len()) as f32 * DISSIMILARITY_PERCENT
        })
        .map(|(_, e)| (e, MatchTag::Fuzzy))
}

/// Smallest distance from `target` to the endpoint uri under each service url, or bare.
fn fuzzy_distance(e: &Placed, target: &str) -> usize {
    let path = e.target.trim_start_matches('/');
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
fn drop_dominated<'a>(c: &[&str], found: Vec<Found<'a, '_>>) -> Vec<(&'a Placed, MatchTag)> {
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
fn sort_key(e: &Placed) -> (&str, &str, &str, &str) {
    (&e.service.name, &e.target, e.function_name(), e.file())
}

#[cfg(test)]
mod tests;
