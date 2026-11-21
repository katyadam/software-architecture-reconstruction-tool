//! Static matcher: initiators -> receivers into `Matches`, no model.
//! Points come from every protocol; only HTTP has matching rules so far.
//! Every HTTP candidate becomes a link, as the synthesizer baseline does.

mod group;
mod http;
mod metrics;
mod points;
mod services;

use std::collections::BTreeMap;

use agentic_models::{
    Address, CodeElement, ConnectionPoint, Link, Matches, Protocol, Sdg, Service, ServiceId,
    Unmatched, UnmatchedReason,
};
use models::ConfigurationData;
use models::configuration::ServiceDescription;
use models::ir::evaluted::EvaluatedIR;
use models::substitute::Substituter;

pub use metrics::{Metrics, ProtocolMetrics};
use services::Services;

/// Matches the initiators of `ir` to its receivers; `constants` are `(name, value)` pairs.
pub fn match_static(
    ir: &EvaluatedIR,
    config: &ConfigurationData,
    constants: &[(String, String)],
) -> Result<(Matches, Metrics), regex::Error> {
    let config = &config.service_descriptions;
    let services = Services::new(config);
    let substituter = Substituter::new(constants)?;
    let mut metrics = Metrics::default();
    let points = points::build(ir, config, &substituter, &mut metrics);

    let mut by_protocol: BTreeMap<Protocol, Vec<&ConnectionPoint>> = BTreeMap::new();
    for r in &points.receivers {
        metrics.of(r.protocol).receivers += 1;
        by_protocol.entry(r.protocol).or_default().push(r);
    }
    let ctx = Ctx {
        by_protocol,
        services: &services,
        substituter: &substituter,
    };
    let mut links = Vec::new();
    let mut unmatched = Vec::new();
    for call in &points.initiators {
        match_one(call, &ctx, &mut metrics, &mut links, &mut unmatched);
    }

    let dependencies = group::dependencies(links);
    metrics.dependencies = dependencies.len();
    let sdg = Sdg {
        services: sdg_services(config, &points.receivers),
        dependencies,
    };
    let matches = Matches {
        sdg,
        unmatched,
        excluded: vec![],
    };
    Ok((matches, metrics))
}

/// What every initiator is matched against.
struct Ctx<'a> {
    by_protocol: BTreeMap<Protocol, Vec<&'a ConnectionPoint>>,
    services: &'a Services<'a>,
    substituter: &'a Substituter,
}

/// Dispatches one initiator on its address kind.
fn match_one(
    call: &ConnectionPoint,
    ctx: &Ctx,
    metrics: &mut Metrics,
    links: &mut Vec<(ServiceId, Link)>,
    unmatched: &mut Vec<Unmatched>,
) {
    let protocol = call.protocol;
    let receivers = ctx
        .by_protocol
        .get(&protocol)
        .map_or(&[][..], Vec::as_slice);
    match (&call.address, &call.element) {
        (Address::Http { .. }, CodeElement::RestCall(c)) => {
            let target = ctx.substituter.apply(&c.target_uri);
            let found = http::candidates(call, &target, receivers, ctx.services);
            let found: Vec<&ConnectionPoint> = found.into_iter().map(|(r, _)| r).collect();
            metrics.record(protocol, &found);
            if found.is_empty() {
                unmatched.push(group::unmatched(
                    call,
                    UnmatchedReason::NoCandidate,
                    String::new(),
                ));
            }
            for r in found {
                let kind = http::request_kind(call, &target, r, ctx.services);
                links.push((r.service.clone(), group::link(call, r, kind)));
            }
        }
        (Address::Unresolved { .. }, _) => {
            metrics.unresolved(protocol);
            unmatched.push(group::unmatched(
                call,
                UnmatchedReason::AddressUnresolved,
                String::new(),
            ));
        }
        _ => {
            metrics.no_rules(protocol);
            let note = format!("no rules for {protocol:?}");
            unmatched.push(group::unmatched(call, UnmatchedReason::NoCandidate, note));
        }
    }
}

/// One service per name with its endpoints, sorted by name. Like the
/// baseline, the last config entry of a name wins and endpoints' services are
/// added, with no urls, when no config entry has them.
fn sdg_services(config: &[ServiceDescription], receivers: &[ConnectionPoint]) -> Vec<Service> {
    let service = |name: &str, urls: &[String]| Service {
        name: name.to_string(),
        urls: urls.to_vec(),
        endpoints: vec![],
    };
    let mut by_name: BTreeMap<&str, Service> = config
        .iter()
        .map(|sd| (sd.name.as_str(), service(&sd.name, &sd.urls)))
        .collect();
    for r in receivers {
        let CodeElement::Endpoint(e) = &r.element else {
            continue;
        };
        by_name
            .entry(&r.service.0)
            .or_insert_with(|| service(&r.service.0, &[]))
            .endpoints
            .push(e.clone());
    }
    by_name.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::points::tests::{edge, ir};
    use models::{CommunicationProtocol, Endpoint, MessageRole};

    fn svc(name: &str, base: &str) -> ServiceDescription {
        ServiceDescription {
            name: name.into(),
            base_dir_path: base.into(),
            urls: vec![format!("http://{name}:8000")],
            java_analysis: None,
        }
    }

    fn ep(file: &str) -> Endpoint {
        Endpoint {
            uri: "/x".into(),
            file_path: file.into(),
            ..Default::default()
        }
    }

    #[test]
    fn duplicate_names_merge_and_unassigned_kept() {
        let config = ConfigurationData {
            service_descriptions: vec![svc("mds", "p/mds1"), svc("mds", "p/mds2")],
        };
        let eps = vec![ep("p/mds1/a.py"), ep("p/mds2/b.py"), ep("p/tools/c.py")];
        let (m, metrics) = match_static(&ir(eps, vec![], vec![]), &config, &[]).expect("matches");
        let got: Vec<_> = m
            .sdg
            .services
            .iter()
            .map(|s| (s.name.as_str(), s.endpoints.len()))
            .collect();
        assert_eq!(got, vec![("", 1), ("mds", 2)]);
        let http = &metrics.protocols[&Protocol::Http];
        assert_eq!((http.receivers, http.unassigned_receivers), (3, 1));
    }

    #[test]
    fn message_without_rules_is_unmatched() {
        let config = ConfigurationData {
            service_descriptions: vec![svc("a", "a")],
        };
        let producer = edge(
            CommunicationProtocol::Kafka,
            MessageRole::Producer,
            "orders",
        );
        let (m, metrics) =
            match_static(&ir(vec![], vec![], vec![producer]), &config, &[]).expect("matches");
        assert_eq!(m.unmatched.len(), 1);
        assert_eq!(m.unmatched[0].reason, UnmatchedReason::NoCandidate);
        assert_eq!(m.unmatched[0].note, "no rules for Kafka");
        assert_eq!(metrics.protocols[&Protocol::Kafka].no_rules, 1);
        assert!(m.sdg.dependencies.is_empty());
    }
}
