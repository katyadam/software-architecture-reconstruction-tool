//! Candidate fan-out counts: how many calls a model would have to decide.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::candidates::MatchTag;
use crate::points::Placed;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Metrics {
    pub initiators: usize,
    pub no_candidate: usize,
    pub single: usize,
    /// Several candidates, all in one service.
    pub same_service_tie: usize,
    /// Candidates in several services.
    pub cross_service_tie: usize,
    pub receivers: usize,
    pub unassigned_initiators: usize,
    pub unassigned_receivers: usize,
    pub links: usize,
    pub dependencies: usize,
}

impl Metrics {
    /// Counts one initiator by its candidates.
    pub(crate) fn record(&mut self, found: &[(&Placed, MatchTag)]) {
        self.initiators += 1;
        self.links += found.len();
        let services: BTreeSet<&str> = found.iter().map(|(e, _)| e.service.name.as_str()).collect();
        match (found.len(), services.len()) {
            (0, _) => self.no_candidate += 1,
            (1, _) => self.single += 1,
            (_, 1) => self.same_service_tie += 1,
            _ => self.cross_service_tie += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::configuration::ServiceDescription;
    use models::{Endpoint, HttpMethod};

    fn ep(service: &str, uri: &str) -> Placed {
        let e = Endpoint {
            http_method: HttpMethod::GET,
            uri: uri.into(),
            ..Default::default()
        };
        let sd = ServiceDescription {
            name: service.into(),
            ..Default::default()
        };
        Placed::receiver(e, sd)
    }

    #[test]
    fn ties_counted_apart() {
        let (a, b, c) = (
            ep("mds", "/x/{id}"),
            ep("mds", "/x/items"),
            ep("cds", "/x/{id}"),
        );
        let mut m = Metrics::default();
        m.record(&[(&a, MatchTag::Template), (&b, MatchTag::Exact)]);
        m.record(&[(&a, MatchTag::Template), (&c, MatchTag::Template)]);
        m.record(&[(&a, MatchTag::Exact)]);
        m.record(&[]);
        let got = (
            m.same_service_tie,
            m.cross_service_tie,
            m.single,
            m.no_candidate,
        );
        assert_eq!(got, (1, 1, 1, 1));
        assert_eq!((m.initiators, m.links), (4, 5));
    }
}
