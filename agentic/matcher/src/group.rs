//! Links and unmatched points, grouped into dependencies with a kind rollup.

use std::collections::BTreeMap;

use agentic_models::{
    Dependency, InteractionKind, Link, MatchSource, Protocol, ServiceId, Unmatched, UnmatchedReason,
};

use crate::points::Point;

/// One static link from `call` to `receiver`.
pub fn link(call: &Point, receiver: &Point, kind: InteractionKind) -> Link {
    Link {
        initiator: call.point.clone(),
        receiver: Some(receiver.point.clone()),
        kind,
        match_source: MatchSource::Static,
        confidence: None,
        evidence: vec![],
    }
}

pub fn unmatched(call: &Point, reason: UnmatchedReason, note: String) -> Unmatched {
    Unmatched {
        point: call.point.clone(),
        reason,
        tried: vec![],
        banned: vec![],
        note,
        evidence: vec![],
    }
}

/// One dependency per (source, target, protocol), sorted; links sorted by point ids.
/// `target` is explicit, so links without a receiving point keep their service.
pub fn dependencies(links: Vec<(ServiceId, Link)>) -> Vec<Dependency> {
    let mut grouped: BTreeMap<(ServiceId, ServiceId, Protocol), Vec<Link>> = BTreeMap::new();
    for (target, link) in links {
        let key = (
            link.initiator.service.clone(),
            target,
            link.initiator.protocol,
        );
        grouped.entry(key).or_default().push(link);
    }
    grouped
        .into_iter()
        .map(|((source, target, protocol), mut links)| {
            links.sort_by_key(|l| {
                (
                    l.initiator.id.clone(),
                    l.receiver.as_ref().map(|r| r.id.clone()),
                )
            });
            Dependency {
                source,
                target,
                protocol,
                kind: InteractionKind::rollup(links.iter().map(|l| l.kind)),
                links,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::points::tests::edge;
    use crate::points::{endpoint, message, rest_call};
    use agentic_models::Side;
    use models::configuration::ServiceDescription;
    use models::substitute::Substituter;
    use models::{CommunicationProtocol, Endpoint, HttpMethod, MessageRole, RestCall};

    fn svc(name: &str) -> ServiceDescription {
        ServiceDescription {
            name: name.into(),
            base_dir_path: name.into(),
            urls: vec![format!("http://{name}:8000")],
            java_analysis: None,
        }
    }

    fn call(file: &str, target: &str) -> Point {
        let rc = RestCall {
            file_path: file.into(),
            target_uri: target.into(),
            ..Default::default()
        };
        rest_call(rc, svc("a"), target)
    }

    fn ep(service: &str, uri: &str) -> Point {
        let e = Endpoint {
            http_method: HttpMethod::GET,
            uri: uri.into(),
            file_path: format!("{service}/api.py"),
            ..Default::default()
        };
        endpoint(e, svc(service))
    }

    fn links(c: &Point, eps: &[Point], kind: InteractionKind) -> Vec<(ServiceId, Link)> {
        eps.iter()
            .map(|e| (e.point.service.clone(), link(c, e, kind)))
            .collect()
    }

    #[test]
    fn two_candidates_two_links() {
        let c = call("a/client.py", "http://x/v3/cases/1");
        let eps = [ep("mds", "/v3/cases/{id}"), ep("cds", "/v3/cases/{id}")];
        let deps = dependencies(links(&c, &eps, InteractionKind::Business));
        let pairs: Vec<_> = deps
            .iter()
            .map(|d| (d.target.0.as_str(), d.links.len()))
            .collect();
        assert_eq!(pairs, vec![("cds", 1), ("mds", 1)]);
        assert!(
            deps.iter()
                .all(|d| d.links[0].match_source == MatchSource::Static)
        );
    }

    #[test]
    fn rollup_business_wins() {
        let business = call("a/client.py", "http://x/v3/cases");
        let health = call("a/probe.py", "http://x/alive");
        let mut all = links(
            &business,
            &[ep("mds", "/v3/cases")],
            InteractionKind::Business,
        );
        all.extend(links(
            &health,
            &[ep("mds", "/alive")],
            InteractionKind::HealthInfra,
        ));
        let deps = dependencies(all);
        assert_eq!(deps[0].kind, InteractionKind::Business);
    }

    #[test]
    fn protocols_kept_apart() {
        use CommunicationProtocol::RabbitMq;
        let http = call("a/client.py", "http://b/x");
        let none = Substituter::new(&[]).expect("no constants");
        let producer = message(
            edge(RabbitMq, MessageRole::Producer, "q"),
            Side::Initiator,
            svc("a"),
            &none,
        );
        let consumer = message(
            edge(RabbitMq, MessageRole::Consumer, "q"),
            Side::Receiver,
            svc("b"),
            &none,
        );
        let mut all = links(&http, &[ep("b", "/x")], InteractionKind::Business);
        all.push((
            consumer.point.service.clone(),
            link(&producer, &consumer, InteractionKind::Business),
        ));
        let got: Vec<_> = dependencies(all)
            .iter()
            .map(|d| (d.target.0.clone(), d.protocol))
            .collect();
        assert_eq!(
            got,
            vec![
                ("b".into(), Protocol::Http),
                ("b".into(), Protocol::RabbitMq)
            ]
        );
    }
}
