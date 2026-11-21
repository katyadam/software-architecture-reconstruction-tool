//! HTTP adapter: REST call -> endpoint candidates by path segment unification.
//! Copied from `synthesizer/src/sdg/matching.rs`; free to diverge.

mod segments;

use std::collections::BTreeSet;

use agentic_models::{Address, ConnectionPoint, Fact, FactId, InteractionKind};
use agentic_tools::address_key;
use models::HttpMethod;
use models::interaction_kind::{InteractionSignals, classify, host_of, is_reflexive};

use crate::services::Services;
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
}

impl MatchTag {
    pub fn name(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Template => "template",
            Self::Suffix => "suffix",
        }
    }
}

/// The runtime fact behind a static link: `<service> | <address> <- <target>`.
pub fn static_fact(tag: MatchTag, receiver: &ConnectionPoint, target: &str) -> Fact {
    Fact {
        id: FactId("f1".into()),
        tool: "static".into(),
        args: tag.name().into(),
        text: format!(
            "{} | {} <- {target}",
            receiver.service.0,
            address_key(&receiver.address)
        ),
    }
}

/// Endpoints `call` can reach, deterministic order (service name, uri).
pub fn candidates<'a>(
    call: &ConnectionPoint,
    target: &str,
    endpoints: &[&'a ConnectionPoint],
    services: &Services,
) -> Vec<(&'a ConnectionPoint, MatchTag)> {
    let (scope, host_known) = scope(call, target, endpoints, services);
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
    kept.sort_by(|(a, _), (b, _)| sort_key(a).cmp(&sort_key(b)));
    kept
}

/// Candidate with its tag and endpoint path segments.
type Found<'a, 's> = (&'a ConnectionPoint, MatchTag, Vec<&'s str>);

/// Same-method endpoints in the services `call` can reach; true when the host is known.
fn scope<'a>(
    call: &ConnectionPoint,
    target: &str,
    endpoints: &[&'a ConnectionPoint],
    services: &Services,
) -> (Vec<&'a ConnectionPoint>, bool) {
    let own = &call.service;
    let host = host_of(target).to_ascii_lowercase();

    let same_method = || {
        endpoints
            .iter()
            .copied()
            .filter(|e| method(e) == method(call))
    };
    let owns_host = |e: &ConnectionPoint| {
        !host.is_empty()
            && e.service != *own
            && services
                .urls(&e.service)
                .iter()
                .any(|u| host_of(u).eq_ignore_ascii_case(&host))
    };

    if is_reflexive(target, services.urls(own), services.all_urls()) {
        (same_method().filter(|e| e.service == *own).collect(), true)
    } else if same_method().any(owns_host) {
        (same_method().filter(|e| owns_host(e)).collect(), true)
    } else {
        (same_method().filter(|e| e.service != *own).collect(), false)
    }
}

/// Drops Suffix matches when any Exact or Template match exists.
fn drop_suffix_if_full<'a, 's>(mut found: Vec<Found<'a, 's>>) -> Vec<Found<'a, 's>> {
    if found.iter().any(|(_, t, _)| *t != MatchTag::Suffix) {
        found.retain(|(_, t, _)| *t != MatchTag::Suffix);
    }
    found
}

/// Drops candidates whose literal matches are a strict subset of another's.
fn drop_dominated<'a>(
    c: &[&str],
    found: Vec<Found<'a, '_>>,
) -> Vec<(&'a ConnectionPoint, MatchTag)> {
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
fn sort_key(e: &ConnectionPoint) -> (&str, &str, &str, &str) {
    (
        &e.service.0,
        path(e),
        e.element.function_name(),
        e.element.file(),
    )
}

pub fn method(p: &ConnectionPoint) -> Option<HttpMethod> {
    match &p.address {
        Address::Http { method, .. } => *method,
        _ => None,
    }
}

/// The address path: an endpoint's uri, a call's path without host.
pub fn path(p: &ConnectionPoint) -> &str {
    match &p.address {
        Address::Http { path, .. } => path,
        _ => "",
    }
}

/// Copy of synthesizer `request_kind`: classify by the matched endpoint's
/// path, falling back to the call target when there is none.
pub fn request_kind(
    call: &ConnectionPoint,
    target: &str,
    receiver: Option<&ConnectionPoint>,
    services: &Services,
) -> InteractionKind {
    let endpoint_uri = receiver.map_or("", path);
    let target_path = if endpoint_uri.is_empty() {
        target
    } else {
        endpoint_uri
    };
    classify(
        &InteractionSignals {
            caller_file: call.element.file(),
            target_path,
            target_uri: target,
        },
        services.urls(&call.service),
        services.all_urls(),
    )
}

#[cfg(test)]
mod tests;
