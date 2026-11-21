//! Candidate fan-out counts per protocol: how many calls a model would have to decide.

use std::collections::{BTreeMap, BTreeSet};

use agentic_models::{ConnectionPoint, Protocol};
use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Metrics {
    /// Only protocols with at least one extracted element.
    pub protocols: BTreeMap<Protocol, ProtocolMetrics>,
    pub dependencies: usize,
    /// Present only when Jev ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jev: Option<JevMetrics>,
}

/// Jev usage and outcomes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct JevMetrics {
    pub initiators: usize,
    pub sessions: usize,
    /// Read from the log, no model call.
    pub replayed: usize,
    pub model_calls: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub receiver_links: usize,
    /// Links with a target service and no receiver.
    pub service_links: usize,
    /// By reason name.
    pub unmatched: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ProtocolMetrics {
    /// Counts up to `cross_service_tie` describe the static candidates, before
    /// Jev; Jev outcomes are in `Metrics::jev`.
    pub initiators: usize,
    pub no_candidate: usize,
    /// The protocol has no matching rules yet; nothing was tried.
    pub no_rules: usize,
    pub unresolved: usize,
    pub single: usize,
    /// Several candidates, all in one service.
    pub same_service_tie: usize,
    /// Candidates in several services.
    pub cross_service_tie: usize,
    pub receivers: usize,
    /// Bindings and declarations: no points, kept for topology resolution.
    pub topology: usize,
    pub unassigned_initiators: usize,
    pub unassigned_receivers: usize,
    pub links: usize,
}

impl Metrics {
    pub(crate) fn of(&mut self, protocol: Protocol) -> &mut ProtocolMetrics {
        self.protocols.entry(protocol).or_default()
    }

    /// Counts one matched initiator by its candidates.
    pub(crate) fn record(&mut self, protocol: Protocol, found: &[&ConnectionPoint]) {
        let m = self.of(protocol);
        m.initiators += 1;
        let services: BTreeSet<&str> = found.iter().map(|e| e.service.0.as_str()).collect();
        match (found.len(), services.len()) {
            (0, _) => m.no_candidate += 1,
            (1, _) => m.single += 1,
            (_, 1) => m.same_service_tie += 1,
            _ => m.cross_service_tie += 1,
        }
    }

    /// Counts one initiator whose address could not be resolved.
    pub(crate) fn unresolved(&mut self, protocol: Protocol) {
        let m = self.of(protocol);
        m.initiators += 1;
        m.unresolved += 1;
    }

    /// Counts one initiator on a protocol without matching rules.
    pub(crate) fn no_rules(&mut self, protocol: Protocol) {
        let m = self.of(protocol);
        m.initiators += 1;
        m.no_rules += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::points::endpoint;
    use agentic_models::ServiceId;
    use models::{Endpoint, HttpMethod};

    fn ep(service: &str, uri: &str) -> ConnectionPoint {
        let e = Endpoint {
            http_method: HttpMethod::GET,
            uri: uri.into(),
            ..Default::default()
        };
        endpoint(e, ServiceId(service.into()))
    }

    #[test]
    fn ties_counted_apart() {
        let (a, b, c) = (
            ep("mds", "/x/{id}"),
            ep("mds", "/x/items"),
            ep("cds", "/x/{id}"),
        );
        let mut m = Metrics::default();
        m.record(Protocol::Http, &[&a, &b]);
        m.record(Protocol::Http, &[&a, &c]);
        m.record(Protocol::Http, &[&a]);
        m.record(Protocol::Http, &[]);
        let h = &m.protocols[&Protocol::Http];
        let got = (
            h.same_service_tie,
            h.cross_service_tie,
            h.single,
            h.no_candidate,
        );
        assert_eq!(got, (1, 1, 1, 1));
        assert_eq!(h.initiators, 4);
    }
}
