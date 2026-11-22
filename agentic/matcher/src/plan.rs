//! Static pass: candidates per initiator, links for the settled ones, the
//! rest left pending for Jev. `finish` groups everything into `Matches`.

use std::collections::BTreeMap;

use agentic_models::{
    Address, CodeElement, ConnectionPoint, Excluded, Link, Matches, Protocol, Sdg, Service,
    ServiceId, Unmatched, UnmatchedReason,
};
use models::ConfigurationData;
use models::configuration::ServiceDescription;
use models::ir::evaluted::EvaluatedIR;
use models::substitute::Substituter;

use crate::http::{self, MatchTag};
use crate::metrics::Metrics;
use crate::services::Services;
use crate::{group, points};

/// An HTTP initiator the static rules did not settle.
#[derive(Debug, Clone)]
pub struct Pending {
    pub call: ConnectionPoint,
    /// The call target after constant substitution.
    pub target: String,
    pub candidates: Vec<ConnectionPoint>,
}

pub struct Plan {
    pub receivers: Vec<ConnectionPoint>,
    /// Empty unless planned with `jev`; sorted by point id.
    pub pending: Vec<Pending>,
    pub metrics: Metrics,
    links: Vec<(ServiceId, Link)>,
    unmatched: Vec<Unmatched>,
    services: Vec<Service>,
}

/// Static pass. With `jev`, only strong matches are linked; other HTTP
/// initiators go to `pending`.
pub fn plan(
    ir: &EvaluatedIR,
    config: &ConfigurationData,
    constants: &[(String, String)],
    jev: bool,
) -> Result<Plan, regex::Error> {
    let config = &config.service_descriptions;
    let substituter = Substituter::new(constants)?;
    let mut metrics = Metrics::default();
    let points = points::build(ir, config, &substituter, &mut metrics);
    let mut out = Out::default();
    {
        let mut by_protocol: BTreeMap<Protocol, Vec<&ConnectionPoint>> = BTreeMap::new();
        for r in &points.receivers {
            metrics.of(r.protocol).receivers += 1;
            by_protocol.entry(r.protocol).or_default().push(r);
        }
        let services = Services::new(config);
        let ctx = Ctx {
            by_protocol,
            services: &services,
            substituter: &substituter,
            jev,
        };
        for call in &points.initiators {
            match_one(call, &ctx, &mut metrics, &mut out);
        }
    }
    out.pending.sort_by(|a, b| a.call.id.cmp(&b.call.id));
    Ok(Plan {
        services: sdg_services(config, &points.receivers),
        receivers: points.receivers,
        pending: out.pending,
        metrics,
        links: out.links,
        unmatched: out.unmatched,
    })
}

impl Plan {
    /// Adds the Jev results and groups all links into dependencies.
    pub fn finish(
        mut self,
        links: Vec<(ServiceId, Link)>,
        unmatched: Vec<Unmatched>,
        excluded: Vec<Excluded>,
    ) -> (Matches, Metrics) {
        self.links.extend(links);
        self.unmatched.extend(unmatched);
        let dependencies = group::dependencies(self.links);
        let mut metrics = self.metrics;
        for d in &dependencies {
            metrics.of(d.protocol).links += d.links.len();
        }
        metrics.dependencies = dependencies.len();
        let sdg = Sdg {
            services: self.services,
            dependencies,
        };
        let matches = Matches {
            sdg,
            unmatched: self.unmatched,
            excluded,
        };
        (matches, metrics)
    }
}

/// What every initiator is matched against.
struct Ctx<'a> {
    by_protocol: BTreeMap<Protocol, Vec<&'a ConnectionPoint>>,
    services: &'a Services<'a>,
    substituter: &'a Substituter,
    jev: bool,
}

#[derive(Default)]
struct Out {
    links: Vec<(ServiceId, Link)>,
    unmatched: Vec<Unmatched>,
    pending: Vec<Pending>,
}

/// Dispatches one initiator on its address kind.
fn match_one(call: &ConnectionPoint, ctx: &Ctx, metrics: &mut Metrics, out: &mut Out) {
    let protocol = call.protocol;
    match (&call.address, &call.element) {
        (Address::Http { .. }, CodeElement::RestCall(c)) => {
            let target = ctx.substituter.apply(&c.target_uri);
            http_one(call, target, ctx, metrics, out);
        }
        (Address::Unresolved { .. }, _) => {
            metrics.unresolved(protocol);
            let reason = UnmatchedReason::AddressUnresolved;
            out.unmatched
                .push(group::unmatched(call, reason, String::new()));
        }
        _ => {
            metrics.no_rules(protocol);
            let note = format!("no rules for {protocol:?}");
            let reason = UnmatchedReason::NoCandidate;
            out.unmatched.push(group::unmatched(call, reason, note));
        }
    }
}

/// Links a strong match, or every candidate without `jev`; else pending.
fn http_one(
    call: &ConnectionPoint,
    target: String,
    ctx: &Ctx,
    metrics: &mut Metrics,
    out: &mut Out,
) {
    let receivers = ctx
        .by_protocol
        .get(&call.protocol)
        .map_or(&[][..], Vec::as_slice);
    let found = http::candidates(call, &target, receivers, ctx.services);
    let points: Vec<&ConnectionPoint> = found.iter().map(|(r, _)| *r).collect();
    metrics.record(call.protocol, &points);
    let strong = matches!(found[..], [(_, MatchTag::Exact | MatchTag::Template)]);
    if ctx.jev && !strong {
        out.pending.push(Pending {
            call: call.clone(),
            target,
            candidates: points.into_iter().cloned().collect(),
        });
        return;
    }
    if found.is_empty() {
        let reason = UnmatchedReason::NoCandidate;
        out.unmatched
            .push(group::unmatched(call, reason, String::new()));
    }
    for (r, tag) in found {
        let kind = http::request_kind(call, &target, Some(r), ctx.services);
        let fact = http::static_fact(tag, r, &target);
        out.links
            .push((r.service.clone(), group::link(call, r, kind, fact)));
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
