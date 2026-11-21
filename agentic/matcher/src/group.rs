//! Links from candidates, grouped into dependencies with a kind rollup.

use std::collections::BTreeMap;

use agentic_models::{
    Dependency, InteractionKind, Link, MatchSource, Protocol, ServiceId, Unmatched, UnmatchedReason,
};
use models::interaction_kind::{InteractionSignals, classify};

use crate::points::Placed;

/// One static link from `call` to `receiver`.
pub fn link(call: &Placed, receiver: &Placed, all_urls: &[String]) -> Link {
    Link {
        initiator: call.point.clone(),
        receiver: Some(receiver.point.clone()),
        kind: request_kind(call, &receiver.target, all_urls),
        match_source: MatchSource::Static,
        confidence: None,
        evidence: vec![],
    }
}

pub fn no_candidate(call: &Placed) -> Unmatched {
    Unmatched {
        point: call.point.clone(),
        reason: UnmatchedReason::NoCandidate,
        tried: vec![],
        banned: vec![],
        note: String::new(),
        evidence: vec![],
    }
}

/// One dependency per (source, target), sorted; links sorted by point ids.
pub fn dependencies(links: Vec<(ServiceId, Link)>) -> Vec<Dependency> {
    let mut grouped: BTreeMap<(ServiceId, ServiceId), Vec<Link>> = BTreeMap::new();
    for (target, link) in links {
        let source = link.initiator.service.clone();
        grouped.entry((source, target)).or_default().push(link);
    }
    grouped
        .into_iter()
        .map(|((source, target), mut links)| {
            links.sort_by_key(|l| {
                (
                    l.initiator.id.clone(),
                    l.receiver.as_ref().map(|r| r.id.clone()),
                )
            });
            Dependency {
                source,
                target,
                protocol: Protocol::Http,
                kind: InteractionKind::rollup(links.iter().map(|l| l.kind)),
                links,
            }
        })
        .collect()
}

/// Copy of synthesizer `request_kind`: classify by the matched endpoint's
/// path, falling back to the call target when there is none.
fn request_kind(call: &Placed, endpoint_uri: &str, all_urls: &[String]) -> InteractionKind {
    let target_path = if endpoint_uri.is_empty() {
        call.target.as_str()
    } else {
        endpoint_uri
    };
    classify(
        &InteractionSignals {
            caller_file: call.file(),
            target_path,
            target_uri: &call.target,
        },
        &call.service.urls,
        all_urls,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::configuration::ServiceDescription;
    use models::{Endpoint, HttpMethod, RestCall};

    fn svc(name: &str) -> ServiceDescription {
        ServiceDescription {
            name: name.into(),
            base_dir_path: name.into(),
            urls: vec![format!("http://{name}:8000")],
            java_analysis: None,
        }
    }

    fn call(file: &str, target: &str) -> Placed {
        let rc = RestCall {
            file_path: file.into(),
            target_uri: target.into(),
            ..Default::default()
        };
        Placed::initiator(rc, svc("a"), target.into())
    }

    fn ep(service: &str, uri: &str) -> Placed {
        let e = Endpoint {
            http_method: HttpMethod::GET,
            uri: uri.into(),
            file_path: format!("{service}/api.py"),
            ..Default::default()
        };
        Placed::receiver(e, svc(service))
    }

    fn links(c: &Placed, eps: &[Placed]) -> Vec<(ServiceId, Link)> {
        eps.iter()
            .map(|e| (e.point.service.clone(), link(c, e, &[])))
            .collect()
    }

    #[test]
    fn two_candidates_two_links() {
        let c = call("a/client.py", "http://x/v3/cases/1");
        let deps = dependencies(links(
            &c,
            &[ep("mds", "/v3/cases/{id}"), ep("cds", "/v3/cases/{id}")],
        ));
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
        let mut all = links(&business, &[ep("mds", "/v3/cases")]);
        all.extend(links(&health, &[ep("mds", "/alive")]));
        let deps = dependencies(all);
        let kinds: Vec<_> = deps[0].links.iter().map(|l| l.kind).collect();
        assert!(kinds.contains(&InteractionKind::HealthInfra));
        assert_eq!(deps[0].kind, InteractionKind::Business);
    }

    #[test]
    fn test_call_is_test_origin() {
        let c = call("a/tests/test_client.py", "http://x/v3/cases");
        assert_eq!(
            link(&c, &ep("mds", "/v3/cases"), &[]).kind,
            InteractionKind::TestOrigin
        );
    }

    #[test]
    fn no_candidate_reason() {
        let u = no_candidate(&call("a/client.py", "/nowhere"));
        assert_eq!(u.reason, UnmatchedReason::NoCandidate);
    }
}
