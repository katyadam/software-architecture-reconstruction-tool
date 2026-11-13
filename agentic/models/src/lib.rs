//! Output of the agentic SDG stages: matcher, verifier and rematcher all
//! take and return [`Matches`].

use models::{CommunicationProtocol, Endpoint, HttpMethod, MessageEdge, RestCall};
use serde::{Deserialize, Serialize};

pub use models::InteractionKind;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Matches {
    pub sdg: Sdg,
    pub unmatched: Vec<Unmatched>,
    /// Points deliberately left out of the SDG, kept so a run can audit them.
    pub excluded: Vec<Excluded>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Sdg {
    pub services: Vec<Service>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    pub endpoints: Vec<Endpoint>,
    pub urls: Vec<String>,
}

/// Keyed by `(source, target, protocol)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dependency {
    pub source: ServiceId,
    pub target: ServiceId,
    pub protocol: Protocol,
    /// Rollup over `links`; `Business` wins any tie.
    pub kind: InteractionKind,
    pub links: Vec<Link>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub initiator: ConnectionPoint,
    /// `None`: the target service is known, the receiving point is not.
    pub receiver: Option<ConnectionPoint>,
    pub kind: InteractionKind,
    pub match_source: MatchSource,
    pub confidence: Option<f32>,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchSource {
    Static,
    Jev,
    SystemTwo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Unmatched {
    pub point: ConnectionPoint,
    pub reason: UnmatchedReason,
    /// Candidates the model saw.
    pub tried: Vec<PointId>,
    /// Receivers the verifier dropped; rematch must not pick them again.
    pub banned: Vec<PointId>,
    /// The model's explanation, or the verdict reason.
    pub note: String,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnmatchedReason {
    AddressUnresolved,
    NoCandidate,
    NoFit,
    Ambiguous,
    BudgetExhausted,
    DroppedByVerifier,
    /// Provider failure (timeout, error status, unparsable answer), not a model decision.
    ModelError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Excluded {
    pub point: ConnectionPoint,
    pub reason: ExcludedReason,
    pub note: String,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExcludedReason {
    /// Talks to a system outside the analysed project, e.g. a third-party API.
    External,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionPoint {
    pub id: PointId,
    pub service: ServiceId,
    pub side: Side,
    pub protocol: Protocol,
    /// Normalized; resolution may update it.
    pub address: Address,
    /// The original extraction, never mutated.
    pub element: CodeElement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Initiator,
    Receiver,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Address {
    Http {
        method: Option<HttpMethod>,
        host: Option<String>,
        path: String,
    },
    Rpc {
        service: String,
        method: String,
    },
    Channel {
        name: String,
        broker: Option<String>,
    },
    Resource {
        name: String,
    },
    Unresolved {
        raw: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodeElement {
    RestCall(RestCall),
    Endpoint(Endpoint),
    Message(MessageEdge),
}

impl CodeElement {
    /// gRPC arrives as a `MessageEdge`, so the protocol comes from the edge.
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::RestCall(_) | Self::Endpoint(_) => Protocol::Http,
            Self::Message(edge) => match edge.protocol {
                CommunicationProtocol::Grpc => Protocol::Grpc,
                CommunicationProtocol::Kafka => Protocol::Kafka,
                CommunicationProtocol::RabbitMq => Protocol::RabbitMq,
            },
        }
    }

    /// Stable across runs: no line numbers, which most elements lack.
    pub fn point_id(&self) -> PointId {
        let (kind, file, function_hash, raw, span) = match self {
            Self::RestCall(c) => (
                "rest_call",
                &c.file_path,
                &c.function_hash,
                c.target_uri.as_str(),
                Some((c.source_span.start_byte, c.source_span.end_byte)),
            ),
            Self::Endpoint(e) => (
                "endpoint",
                &e.file_path,
                &e.function_hash,
                e.uri.as_str(),
                None,
            ),
            Self::Message(m) => (
                "message",
                &m.file_path,
                &m.function_hash,
                m.destination.as_str(),
                None,
            ),
        };
        let mut id = format!("{file}#{function_hash}#{kind}#{raw}");
        if let Some((start, end)) = span.filter(|(start, end)| start != end) {
            id.push_str(&format!("#{start}-{end}"));
        }
        PointId(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    Http,
    Grpc,
    Kafka,
    RabbitMq,
}

/// One claim, backed by one tool result the runtime recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub fact: Fact,
    pub claim: Claim,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fact {
    pub id: FactId,
    pub tool: String,
    pub args: String,
    /// Tool output, written by the runtime, never by the model.
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Claim {
    Address,
    TargetService,
    Receiver,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PointId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ServiceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FactId(pub String);

#[cfg(test)]
mod tests {
    use super::*;
    use models::source_code::SourceSpan;

    fn call(start: u32, end: u32) -> CodeElement {
        CodeElement::RestCall(RestCall {
            file_path: "a/Client.java".into(),
            function_hash: "h1".into(),
            target_uri: "http://orders/api".into(),
            source_span: SourceSpan {
                start_byte: start,
                end_byte: end,
            },
            ..Default::default()
        })
    }

    #[test]
    fn point_id_separates_calls_in_one_function_by_span() {
        assert_ne!(call(10, 20).point_id(), call(30, 40).point_id());
        assert_eq!(call(10, 20).point_id(), call(10, 20).point_id());
        assert_eq!(
            call(0, 0).point_id().0,
            "a/Client.java#h1#rest_call#http://orders/api"
        );
    }

    #[test]
    fn matches_round_trips_through_json() {
        let element = call(10, 20);
        let point = ConnectionPoint {
            id: element.point_id(),
            service: ServiceId("orders-client".into()),
            side: Side::Initiator,
            protocol: element.protocol(),
            address: Address::Unresolved {
                raw: "http://orders/api".into(),
            },
            element,
        };
        let matches = Matches {
            unmatched: vec![Unmatched {
                point,
                reason: UnmatchedReason::ModelError,
                tried: vec![],
                banned: vec![],
                note: "timeout".into(),
                evidence: vec![],
            }],
            ..Default::default()
        };
        let json = serde_json::to_string(&matches).unwrap();
        assert_eq!(serde_json::from_str::<Matches>(&json).unwrap(), matches);
    }
}
