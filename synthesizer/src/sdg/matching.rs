//! REST call -> endpoint matching by path segment unification.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use strsim::levenshtein;
use utoipa::ToSchema;

use crate::sdg::interaction_kind::{host_of, is_reflexive};
use crate::sdg::model::{AssignedEndpoint, AssignedRestCall};

/// How a request's endpoint was chosen.
#[derive(
    Debug, Serialize, Deserialize, ToSchema, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum MatchTag {
    /// All segments literal and equal.
    #[default]
    Exact,
    /// Same length, at least one variable segment.
    Template,
    /// Matched on a trailing run of segments only.
    Suffix,
    /// Levenshtein fallback, no unified candidate.
    Fuzzy,
    /// Kept by the Jev tie-break.
    Jev,
}

/// Endpoints `call` can reach, deterministic order (service name, uri).
pub fn candidates<'a>(
    call: &AssignedRestCall,
    endpoints: &'a [AssignedEndpoint],
    all_urls: &[String],
) -> Vec<(&'a AssignedEndpoint, MatchTag)> {
    let (scope, host_known) = scope(call, endpoints, all_urls);
    let target = &call.data.target_uri;
    let c = segments(target);
    let leading_hole = !target.contains("://") && c.first().is_some_and(|s| is_var(s));

    let found = scope
        .iter()
        .filter_map(|e| {
            let s = segments(&e.data.uri);
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
type Found<'a, 's> = (&'a AssignedEndpoint, MatchTag, Vec<&'s str>);

/// Same-method endpoints in the services `call` can reach; true when the host is known.
fn scope<'a>(
    call: &AssignedRestCall,
    endpoints: &'a [AssignedEndpoint],
    all_urls: &[String],
) -> (Vec<&'a AssignedEndpoint>, bool) {
    let target = &call.data.target_uri;
    let own = &call.service;
    let host = host_of(target).to_ascii_lowercase();

    let same_method = || {
        endpoints
            .iter()
            .filter(|e| e.data.http_method == call.data.http_method)
    };
    let owns_host = |e: &AssignedEndpoint| {
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

/// Tag for call segments `c` against endpoint segments `s`, `None` if they do not match.
fn match_tag(c: &[&str], s: &[&str], leading_hole: bool, host_known: bool) -> Option<MatchTag> {
    if c.len() == s.len() && unifies(c, s) {
        if c.iter().chain(s).any(|x| is_var(x)) {
            Some(MatchTag::Template)
        } else {
            Some(MatchTag::Exact)
        }
    } else if (leading_hole && s.len() + 1 >= c.len() && unifies(&c[1..], tail(s, c.len() - 1)))
        // Lost router prefix, e.g. `app.mount("/v3")`.
        || (host_known && s.len() < c.len() && unifies(tail(c, s.len()), s))
    {
        Some(MatchTag::Suffix)
    } else {
        None
    }
}

/// True when some aligned position is literal on both sides; `{a}/{b}` unifies with anything.
fn has_evidence(c: &[&str], s: &[&str]) -> bool {
    !literals(c, s).is_empty()
}

/// Old Levenshtein argmin over `scope`, kept within `DISSIMILARITY_PERCENT`; ties -> smallest `sort_key`.
fn fuzzy_fallback<'a>(
    target: &str,
    scope: &[&'a AssignedEndpoint],
) -> Option<(&'a AssignedEndpoint, MatchTag)> {
    scope
        .iter()
        .map(|e| (fuzzy_distance(e, target), *e))
        .min_by(|(da, a), (db, b)| da.cmp(db).then_with(|| sort_key(a).cmp(&sort_key(b))))
        .filter(|(d, e)| {
            (*d as f32) < e.data.uri.len().max(target.len()) as f32 * DISSIMILARITY_PERCENT
        })
        .map(|(_, e)| (e, MatchTag::Fuzzy))
}

/// Smallest distance from `target` to the endpoint uri under each service url, or bare.
fn fuzzy_distance(e: &AssignedEndpoint, target: &str) -> usize {
    let path = e.data.uri.trim_start_matches('/');
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
fn drop_dominated<'a>(
    c: &[&str],
    found: Vec<Found<'a, '_>>,
) -> Vec<(&'a AssignedEndpoint, MatchTag)> {
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
fn sort_key(e: &AssignedEndpoint) -> (&String, &String, &String, &String) {
    (
        &e.service.name,
        &e.data.uri,
        &e.data.function_name,
        &e.data.file_path,
    )
}

/// Path segments without scheme, authority, query or fragment.
fn segments(uri: &str) -> Vec<&str> {
    let path = uri.split(['?', '#']).next().unwrap_or_default();
    let path = match path.split_once("://") {
        Some((_, rest)) => rest.split_once('/').map_or("", |(_, p)| p),
        None => path,
    };
    path.split('/').filter(|s| !s.is_empty()).collect()
}

fn is_var(seg: &str) -> bool {
    seg.contains('{') || seg.starts_with(':') || seg == "*" || seg == "**"
}

fn unifies(a: &[&str], b: &[&str]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| is_var(x) || is_var(y) || x == y)
}

fn tail<'s, 'a>(segs: &'s [&'a str], n: usize) -> &'s [&'a str] {
    &segs[segs.len().saturating_sub(n)..]
}

/// Positions, counted from the end, where both `c` and `s` are literal.
fn literals(c: &[&str], s: &[&str]) -> BTreeSet<usize> {
    c.iter()
        .rev()
        .zip(s.iter().rev())
        .enumerate()
        .filter(|(_, (x, y))| !is_var(x) && !is_var(y))
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::{Endpoint, HttpMethod, RestCall, configuration::ServiceDescription};

    fn svc(name: &str, port: u16) -> ServiceDescription {
        ServiceDescription {
            name: name.into(),
            base_dir_path: String::new(),
            urls: vec![format!("http://{name}:{port}")],
        }
    }

    fn ep(service: &ServiceDescription, method: HttpMethod, uri: &str) -> AssignedEndpoint {
        AssignedEndpoint::new(
            Endpoint {
                http_method: method,
                uri: uri.into(),
                ..Default::default()
            },
            service.clone(),
        )
    }

    fn call(method: HttpMethod, target: &str) -> AssignedRestCall {
        AssignedRestCall {
            data: RestCall {
                http_method: method,
                target_uri: target.into(),
                ..Default::default()
            },
            service: svc("ts-basic-service", 15680),
        }
    }

    fn all_urls(endpoints: &[AssignedEndpoint]) -> Vec<String> {
        let mut urls: Vec<String> = endpoints
            .iter()
            .flat_map(|e| e.service.urls.clone())
            .chain(svc("ts-basic-service", 15680).urls)
            .collect();
        urls.sort();
        urls.dedup();
        urls
    }

    fn run(call: &AssignedRestCall, endpoints: &[AssignedEndpoint]) -> Vec<(String, MatchTag)> {
        candidates(call, endpoints, &all_urls(endpoints))
            .into_iter()
            .map(|(e, t)| (e.data.uri.clone(), t))
            .collect()
    }

    fn station() -> ServiceDescription {
        svc("ts-station-service", 12345)
    }

    fn order() -> ServiceDescription {
        svc("ts-order-service", 12031)
    }

    const ORDER: &str = "http://ts-order-service:12031/api/v1/orderservice/order";

    #[test]
    fn literal_path_beats_shorter_collection() {
        let s = station();
        let eps = [
            ep(&s, HttpMethod::GET, "/api/v1/stationservice/stations"),
            ep(
                &s,
                HttpMethod::GET,
                "/api/v1/stationservice/stations/id/{stationNameForId}",
            ),
        ];
        let c = call(
            HttpMethod::GET,
            "http://ts-station-service:12345/api/v1/stationservice/stations/id/{end_place}",
        );
        assert_eq!(
            run(&c, &eps),
            vec![(
                "/api/v1/stationservice/stations/id/{stationNameForId}".into(),
                MatchTag::Template
            )]
        );
    }

    #[test]
    fn order_status_hole_picks_status_endpoint() {
        let o = order();
        let eps = [
            ep(
                &o,
                HttpMethod::GET,
                "/api/v1/orderservice/order/price/{orderId}",
            ),
            ep(
                &o,
                HttpMethod::GET,
                "/api/v1/orderservice/order/status/{orderId}/{status}",
            ),
        ];
        let c = call(
            HttpMethod::GET,
            &format!("{ORDER}/status/{{orderId}}/{{status}}"),
        );
        assert_eq!(
            run(&c, &eps),
            vec![(
                "/api/v1/orderservice/order/status/{orderId}/{status}".into(),
                MatchTag::Template
            )]
        );
    }

    #[test]
    fn literal_dominates_template() {
        let o = order();
        let eps = [
            ep(&o, HttpMethod::GET, "/api/v1/orderservice/order/tickets"),
            ep(&o, HttpMethod::GET, "/api/v1/orderservice/order/{id}"),
        ];
        let c = call(HttpMethod::GET, &format!("{ORDER}/tickets"));
        assert_eq!(
            run(&c, &eps),
            vec![("/api/v1/orderservice/order/tickets".into(), MatchTag::Exact)]
        );
    }

    #[test]
    fn hole_keeps_both_literal_and_template() {
        let o = order();
        let eps = [
            ep(&o, HttpMethod::GET, "/api/v1/orderservice/order/tickets"),
            ep(&o, HttpMethod::GET, "/api/v1/orderservice/order/{id}"),
        ];
        let c = call(HttpMethod::GET, &format!("{ORDER}/{{x}}"));
        assert_eq!(
            run(&c, &eps),
            vec![
                (
                    "/api/v1/orderservice/order/tickets".into(),
                    MatchTag::Template
                ),
                ("/api/v1/orderservice/order/{id}".into(), MatchTag::Template),
            ]
        );
    }

    #[test]
    fn host_restricts_to_service() {
        let consign = svc("ts-consign-service", 16111);
        let config = svc("ts-config-service", 15679);
        let eps = [
            ep(
                &consign,
                HttpMethod::POST,
                "/api/v1/consignservice/consigns",
            ),
            ep(&config, HttpMethod::POST, "/api/v1/configservice/configs"),
        ];
        let c = call(
            HttpMethod::POST,
            "http://ts-consign-service:16111/api/v1/consignservice/consigns",
        );
        let got = candidates(&c, &eps, &all_urls(&eps));
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0.service.name, "ts-consign-service");
        assert_eq!(got[0].1, MatchTag::Exact);
    }

    #[test]
    fn lost_prefix_matches_as_suffix_when_host_known() {
        let eps = [ep(
            &svc("clinical-data-service", 8000),
            HttpMethod::GET,
            "/cases/{case_id}",
        )];
        let c = call(
            HttpMethod::GET,
            "http://clinical-data-service:8000/v3/cases/{id}",
        );
        assert_eq!(
            run(&c, &eps),
            vec![("/cases/{case_id}".into(), MatchTag::Suffix)]
        );
    }

    #[test]
    fn lost_prefix_ignored_when_host_unknown() {
        let eps = [ep(
            &svc("clinical-data-service", 8000),
            HttpMethod::GET,
            "/cases/{case_id}",
        )];
        let c = call(HttpMethod::GET, "/v3/cases/{id}");
        assert!(run(&c, &eps).is_empty());
    }

    #[test]
    fn leading_hole_suffix_matches_any_service() {
        let eps = [ep(
            &svc("ts-preserve-service", 14568),
            HttpMethod::GET,
            "/api/v1/preserveservice/preserve",
        )];
        let c = call(HttpMethod::GET, "{url}/api/v1/preserveservice/preserve");
        assert_eq!(
            run(&c, &eps),
            vec![("/api/v1/preserveservice/preserve".into(), MatchTag::Suffix)]
        );
    }

    #[test]
    fn unconfigured_host_falls_back_to_full_path_match() {
        let eps = [ep(
            &station(),
            HttpMethod::GET,
            "/api/v1/stationservice/stations",
        )];
        let c = call(
            HttpMethod::GET,
            "http://ts-station-service.default.svc.cluster.local/api/v1/stationservice/stations",
        );
        assert_eq!(
            run(&c, &eps),
            vec![("/api/v1/stationservice/stations".into(), MatchTag::Exact)]
        );
    }

    #[test]
    fn method_mismatch_excluded() {
        let eps = [ep(
            &station(),
            HttpMethod::GET,
            "/api/v1/stationservice/stations",
        )];
        let c = call(
            HttpMethod::POST,
            "http://ts-station-service:12345/api/v1/stationservice/stations",
        );
        assert!(run(&c, &eps).is_empty());
    }

    #[test]
    fn output_independent_of_endpoint_order() {
        let (o, s) = (order(), station());
        let mut eps = vec![
            ep(&o, HttpMethod::GET, "/api/v1/orderservice/order/{id}"),
            ep(&o, HttpMethod::GET, "/api/v1/orderservice/order/tickets"),
            ep(&s, HttpMethod::GET, "/api/v1/orderservice/order/{id}"),
            ep(&s, HttpMethod::GET, "/api/v1/stationservice/stations"),
        ];
        // Unconfigured host -> every other service in scope, ties across services.
        let c = call(
            HttpMethod::GET,
            "http://alias/api/v1/orderservice/order/{x}",
        );
        let named = |eps: &[AssignedEndpoint]| -> Vec<(String, String, MatchTag)> {
            candidates(&c, eps, &all_urls(eps))
                .into_iter()
                .map(|(e, t)| (e.service.name.clone(), e.data.uri.clone(), t))
                .collect()
        };
        let forward = named(&eps);
        eps.reverse();
        assert_eq!(forward, named(&eps));
        assert_eq!(forward.len(), 3);
    }
    #[test]
    fn pure_hole_call_has_no_candidates() {
        let (consign, config) = (
            svc("ts-consign-service", 16111),
            svc("ts-config-service", 15679),
        );
        let eps = [
            ep(
                &consign,
                HttpMethod::POST,
                "/api/v1/consignservice/consigns",
            ),
            ep(
                &consign,
                HttpMethod::POST,
                "/api/v1/consignservice/consigns/{id}",
            ),
            ep(&config, HttpMethod::POST, "/api/v1/configservice/configs"),
        ];
        assert!(run(&call(HttpMethod::POST, "{requestUrl}"), &eps).is_empty());
    }

    #[test]
    fn two_hole_call_has_no_candidates() {
        let eps = [ep(&station(), HttpMethod::GET, "/x/{id}")];
        assert!(run(&call(HttpMethod::GET, "{a}/{b}"), &eps).is_empty());
    }

    #[test]
    fn glued_host_falls_back_to_fuzzy() {
        let mds = svc("medical-data-service", 5000);
        let eps = [
            ep(&mds, HttpMethod::GET, "/jobs"),
            ep(&mds, HttpMethod::GET, "/scopes/{id}/x"),
        ];
        let c = call(HttpMethod::GET, "http://medical-data-service:5000url");
        assert_eq!(run(&c, &eps), vec![("/jobs".into(), MatchTag::Fuzzy)]);
    }

    #[test]
    fn fuzzy_rejects_beyond_threshold() {
        let eps = [ep(
            &svc("ts-route-service", 11178),
            HttpMethod::GET,
            "/api/v1/routeservice/routes",
        )];
        assert!(run(&call(HttpMethod::GET, "{requestUrl}"), &eps).is_empty());
    }
}
