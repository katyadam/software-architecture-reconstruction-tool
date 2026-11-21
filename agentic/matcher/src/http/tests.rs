use super::*;
use crate::points::{endpoint, rest_call};
use agentic_models::{CodeElement, ServiceId};
use models::{Endpoint, HttpMethod, RestCall, configuration::ServiceDescription};

fn svc(name: &str, port: u16) -> ServiceDescription {
    ServiceDescription {
        java_analysis: None,
        name: name.into(),
        base_dir_path: String::new(),
        urls: vec![format!("http://{name}:{port}")],
    }
}

/// An endpoint point with the config entry of its service.
type Ep = (ConnectionPoint, ServiceDescription);

fn ep(service: &ServiceDescription, method: HttpMethod, uri: &str) -> Ep {
    let e = Endpoint {
        http_method: method,
        uri: uri.into(),
        ..Default::default()
    };
    (
        endpoint(e, ServiceId(service.name.clone())),
        service.clone(),
    )
}

fn call(method: HttpMethod, target: &str) -> ConnectionPoint {
    let data = RestCall {
        http_method: method,
        target_uri: target.into(),
        ..Default::default()
    };
    rest_call(data, ServiceId("ts-basic-service".into()), target)
}

fn target(call: &ConnectionPoint) -> &str {
    match &call.element {
        CodeElement::RestCall(c) => &c.target_uri,
        _ => "",
    }
}

fn refs(eps: &[Ep]) -> Vec<&ConnectionPoint> {
    eps.iter().map(|(p, _)| p).collect()
}

/// The endpoints' services plus the caller's.
fn config(eps: &[Ep]) -> Vec<ServiceDescription> {
    eps.iter()
        .map(|(_, sd)| sd.clone())
        .chain([svc("ts-basic-service", 15680)])
        .collect()
}

fn found<'a>(call: &ConnectionPoint, eps: &'a [Ep]) -> Vec<(&'a ConnectionPoint, MatchTag)> {
    let cfg = config(eps);
    candidates(call, target(call), &refs(eps), &Services::new(&cfg))
}

fn run(call: &ConnectionPoint, eps: &[Ep]) -> Vec<(String, MatchTag)> {
    found(call, eps)
        .into_iter()
        .map(|(e, t)| (path(e).to_string(), t))
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
    let got = found(&c, &eps);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0.service.0, "ts-consign-service");
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
    let named = |eps: &[Ep]| -> Vec<(String, String, MatchTag)> {
        found(&c, eps)
            .into_iter()
            .map(|(e, t)| (e.service.0.clone(), path(e).to_string(), t))
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
fn no_fuzzy_guess() {
    let mds = svc("medical-data-service", 5000);
    let eps = [ep(&mds, HttpMethod::GET, "/v3/cases")];
    assert!(run(&call(HttpMethod::GET, "/v3/case"), &eps).is_empty());
    let glued = call(HttpMethod::GET, "http://medical-data-service:5000url");
    assert!(run(&glued, &[ep(&mds, HttpMethod::GET, "/jobs")]).is_empty());
}

#[test]
fn static_link_fact() {
    let mds = svc("medical-data-service", 8000);
    let eps = [ep(&mds, HttpMethod::GET, "/v3/cases/{case_id}")];
    let target = "http://medical-data-service:8000/v3/cases/{id}";
    let c = call(HttpMethod::GET, target);
    let [(r, tag)] = found(&c, &eps)[..] else {
        panic!("one candidate");
    };
    let fact = static_fact(tag, r, target);
    assert_eq!((fact.id.0.as_str(), fact.tool.as_str()), ("f1", "static"));
    assert_eq!(fact.args, "template");
    assert_eq!(
        fact.text,
        format!("medical-data-service | GET /v3/cases/{{case_id}} <- {target}")
    );
}
