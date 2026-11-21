//! Static matcher: initiators -> receivers into `Matches`, no model.
//! Points come from every protocol; only HTTP has matching rules so far.
//! Without Jev, every HTTP candidate becomes a link, as the baseline does.

mod group;
mod http;
pub mod jev;
mod metrics;
mod plan;
mod points;
mod services;

use agentic_models::Matches;
use models::ConfigurationData;
use models::ir::evaluted::EvaluatedIR;

pub use http::MatchTag;
pub use metrics::{JevMetrics, Metrics, ProtocolMetrics};
pub use plan::{Pending, Plan, plan};

/// Matches the initiators of `ir` to its receivers; `constants` are `(name, value)` pairs.
pub fn match_static(
    ir: &EvaluatedIR,
    config: &ConfigurationData,
    constants: &[(String, String)],
) -> Result<(Matches, Metrics), regex::Error> {
    Ok(plan(ir, config, constants, false)?.finish(vec![], vec![]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::points::tests::{edge, ir};
    use agentic_models::{Protocol, UnmatchedReason};
    use models::configuration::ServiceDescription;
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

    fn call(file: &str, target: &str) -> models::RestCall {
        models::RestCall {
            file_path: file.into(),
            target_uri: target.into(),
            function_hash: target.into(),
            ..Default::default()
        }
    }

    fn uri(file: &str, uri: &str) -> Endpoint {
        Endpoint {
            uri: uri.into(),
            ..ep(file)
        }
    }

    #[test]
    fn jev_plan_links_only_strong_matches() {
        let config = ConfigurationData {
            service_descriptions: vec![svc("a", "a"), svc("b", "b"), svc("c", "c")],
        };
        let eps = vec![
            uri("b/r.py", "/one/{id}"),
            uri("b/r.py", "/tie/{id}"),
            uri("c/r.py", "/tie/{id}"),
            uri("b/r.py", "/v1/deep/x/tail"),
        ];
        let calls = vec![
            call("a/c.py", "/one/7"),
            call("a/c.py", "/tie/7"),
            call("a/c.py", "{base}/x/tail"),
            call("a/c.py", "/nothing"),
        ];
        let kafka = edge(
            CommunicationProtocol::Kafka,
            MessageRole::Producer,
            "orders",
        );
        let p = plan(&ir(eps, calls, vec![kafka]), &config, &[], true).expect("plan");
        let pending: Vec<_> = p
            .pending
            .iter()
            .map(|x| (x.target.as_str(), x.candidates.len()))
            .collect();
        assert_eq!(
            pending,
            vec![("/nothing", 0), ("/tie/7", 2), ("{base}/x/tail", 1)]
        );
        let jev_link = agentic_models::Link {
            initiator: p.pending[0].call.clone(),
            receiver: None,
            kind: Default::default(),
            match_source: agentic_models::MatchSource::Jev,
            confidence: Some(0.8),
            evidence: vec![],
        };
        let jev = vec![(agentic_models::ServiceId("c".into()), jev_link)];
        let (m, metrics) = p.finish(jev, vec![]);
        let links: Vec<_> = m.sdg.dependencies.iter().flat_map(|d| &d.links).collect();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].evidence[0].args, "template");
        assert_eq!(metrics.protocols[&Protocol::Http].links, 2);
        let unmatched: Vec<_> = m.unmatched.iter().map(|u| u.point.protocol).collect();
        assert_eq!(unmatched, vec![Protocol::Kafka]);
    }
}
