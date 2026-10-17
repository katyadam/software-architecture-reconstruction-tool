//! REST call -> endpoint matching by path segment unification.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
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
    /// Kept by the Jev tie-break.
    Jev,
}

/// Endpoints `call` can reach, deterministic order (service name, uri).
pub fn candidates<'a>(
    call: &AssignedRestCall,
    endpoints: &'a [AssignedEndpoint],
    all_urls: &[String],
) -> Vec<(&'a AssignedEndpoint, MatchTag)> {
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

    let (scope, host_known): (Vec<&AssignedEndpoint>, bool) =
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
        };

    let c = segments(target);
    let leading_hole = !target.contains("://") && c.first().is_some_and(|s| is_var(s));

    let mut found: Vec<(&AssignedEndpoint, MatchTag, Vec<&str>)> = scope
        .into_iter()
        .filter_map(|e| {
            let s = segments(&e.data.uri);
            let tag = if c.len() == s.len() && unifies(&c, &s) {
                if c.iter().chain(&s).any(|x| is_var(x)) {
                    MatchTag::Template
                } else {
                    MatchTag::Exact
                }
            } else if (leading_hole
                && s.len() + 1 >= c.len()
                && unifies(&c[1..], tail(&s, c.len() - 1)))
                // Lost router prefix, e.g. `app.mount("/v3")`.
                || (host_known && s.len() < c.len() && unifies(tail(&c, s.len()), &s))
            {
                MatchTag::Suffix
            } else {
                return None;
            };
            Some((e, tag, s))
        })
        .collect();

    if found.iter().any(|(_, t, _)| *t != MatchTag::Suffix) {
        found.retain(|(_, t, _)| *t != MatchTag::Suffix);
    }

    // ponytail: O(n^2) dominance scan, fine for per-call candidate counts.
    let lits: Vec<BTreeSet<usize>> = found.iter().map(|(_, _, s)| literals(&c, s)).collect();
    let mut kept: Vec<(&AssignedEndpoint, MatchTag)> = found
        .iter()
        .zip(&lits)
        .filter(|(_, b)| !lits.iter().any(|a| a.is_superset(b) && a != *b))
        .map(|((e, t, _), _)| (*e, *t))
        .collect();

    kept.sort_by(|(a, _), (b, _)| {
        (
            &a.service.name,
            &a.data.uri,
            &a.data.function_name,
            &a.data.file_path,
        )
            .cmp(&(
                &b.service.name,
                &b.data.uri,
                &b.data.function_name,
                &b.data.file_path,
            ))
    });
    kept
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
}
