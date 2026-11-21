//! Candidate fan-out counts per protocol: how many calls a model would have to decide.

use std::collections::{BTreeMap, BTreeSet};

use agentic_models::Protocol;
use serde::Serialize;

use crate::points::Point;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Metrics {
    /// Only protocols with at least one extracted element.
    pub protocols: BTreeMap<Protocol, ProtocolMetrics>,
    pub dependencies: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ProtocolMetrics {
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
    pub(crate) fn record(&mut self, protocol: Protocol, found: &[&Point]) {
        let m = self.of(protocol);
        m.initiators += 1;
        m.links += found.len();
        let services: BTreeSet<&str> = found.iter().map(|e| e.service.name.as_str()).collect();
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
    use models::configuration::ServiceDescription;
    use models::{Endpoint, HttpMethod};

    fn ep(service: &str, uri: &str) -> Point {
        let e = Endpoint {
            http_method: HttpMethod::GET,
            uri: uri.into(),
            ..Default::default()
        };
        let sd = ServiceDescription {
            name: service.into(),
            ..Default::default()
        };
        endpoint(e, sd)
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
        assert_eq!((h.initiators, h.links), (4, 5));
    }
}
