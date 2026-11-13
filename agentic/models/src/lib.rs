//! Output of the agentic SDG stages: matcher, verifier and rematcher all
//! take and return [`Matches`].

use models::source_code::SourceSpan;
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

impl Dependency {
    /// Lives here, not on `Link`: a link without a receiver needs `self.target`.
    pub fn link_id(&self, link: &Link) -> LinkId {
        let target = match &link.receiver {
            Some(receiver) => LinkTarget::Receiver(receiver.id.clone()),
            None => LinkTarget::Service(self.target.clone()),
        };
        LinkId {
            initiator: link.initiator.id.clone(),
            target,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LinkId {
    pub initiator: PointId,
    pub target: LinkTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LinkTarget {
    Receiver(PointId),
    Service(ServiceId),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub link: LinkId,
    pub decision: Decision,
    pub reason: String,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    Keep,
    Drop,
    Unverified,
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

impl ConnectionPoint {
    pub fn location(&self) -> Location {
        self.element.location()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub span: Option<SourceSpan>,
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
    /// Endpoints include the method, so `GET /x` and `POST /x` differ.
    pub fn point_id(&self) -> PointId {
        let (kind, function_hash, raw) = match self {
            Self::RestCall(c) => ("rest_call", &c.function_hash, c.target_uri.clone()),
            Self::Endpoint(e) => (
                "endpoint",
                &e.function_hash,
                format!("{:?} {}", e.http_method, e.uri),
            ),
            Self::Message(m) => ("message", &m.function_hash, m.destination.clone()),
        };
        let Location { file, span } = self.location();
        let mut id = format!("{file}#{function_hash}#{kind}#{raw}");
        if let Some(s) = span {
            id.push_str(&format!("#{}-{}", s.start_byte, s.end_byte));
        }
        PointId(id)
    }

    /// Only REST calls carry a span; an empty span counts as none.
    pub fn location(&self) -> Location {
        let (file, span) = match self {
            Self::RestCall(c) => (
                &c.file_path,
                Some(c.source_span.clone()).filter(|s| s.start_byte != s.end_byte),
            ),
            Self::Endpoint(e) => (&e.file_path, None),
            Self::Message(m) => (&m.file_path, None),
        };
        Location {
            file: file.clone(),
            span,
        }
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
    use models::{MessageDestinationKind, MessageRole};

    fn point(element: CodeElement, service: &str, side: Side) -> ConnectionPoint {
        ConnectionPoint {
            id: element.point_id(),
            service: ServiceId(service.into()),
            side,
            protocol: element.protocol(),
            address: Address::Unresolved { raw: "x".into() },
            element,
        }
    }

    fn endpoint(file: &str, uri: &str) -> CodeElement {
        CodeElement::Endpoint(Endpoint {
            file_path: file.into(),
            uri: uri.into(),
            ..Default::default()
        })
    }

    fn link(receiver: Option<ConnectionPoint>) -> Link {
        Link {
            initiator: point(call(10, 20), "orders", Side::Initiator),
            receiver,
            kind: InteractionKind::Business,
            match_source: MatchSource::Jev,
            confidence: None,
            evidence: vec![],
        }
    }

    #[test]
    fn link_id_uses_receiver_or_target_service() {
        let dep = Dependency {
            source: ServiceId("orders".into()),
            target: ServiceId("payments".into()),
            protocol: Protocol::Http,
            kind: InteractionKind::Business,
            links: vec![],
        };
        let initiator = call(10, 20).point_id();
        let a = point(endpoint("p/A.java", "/a"), "payments", Side::Receiver);
        let b = point(endpoint("p/B.java", "/b"), "payments", Side::Receiver);

        let id = |target| LinkId {
            initiator: initiator.clone(),
            target,
        };

        let pair = dep.link_id(&link(Some(a.clone())));
        assert_eq!(pair, id(LinkTarget::Receiver(a.id)));
        assert_eq!(
            dep.link_id(&link(None)),
            id(LinkTarget::Service(ServiceId("payments".into())))
        );
        assert_ne!(pair, dep.link_id(&link(Some(b))));
    }

    #[test]
    fn verdict_round_trips_through_json() {
        let verdict = Verdict {
            link: LinkId {
                initiator: call(10, 20).point_id(),
                target: LinkTarget::Service(ServiceId("payments".into())),
            },
            decision: Decision::Drop,
            reason: "calls billing instead".into(),
            evidence: vec![Evidence {
                fact: Fact {
                    id: FactId("f1".into()),
                    tool: "define".into(),
                    args: "BASE".into(),
                    text: "BASE = \"http://billing\"".into(),
                },
                claim: Claim::TargetService,
            }],
        };
        let json = serde_json::to_string(&verdict).unwrap();
        assert_eq!(serde_json::from_str::<Verdict>(&json).unwrap(), verdict);
    }

    #[test]
    fn location_has_span_only_for_rest_calls_with_one() {
        let at = |element| point(element, "s", Side::Initiator).location();
        let span = SourceSpan {
            start_byte: 10,
            end_byte: 20,
        };
        assert_eq!(
            at(call(10, 20)),
            Location {
                file: "a/Client.java".into(),
                span: Some(span),
            }
        );
        assert_eq!(at(call(0, 0)).span, None);
        assert_eq!(
            at(endpoint("b/Routes.java", "/x")),
            Location {
                file: "b/Routes.java".into(),
                span: None,
            }
        );
    }

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
    fn point_id_separates_methods_on_one_route() {
        let route = |http_method| {
            CodeElement::Endpoint(Endpoint {
                file_path: "app.py".into(),
                function_hash: "h3".into(),
                http_method,
                uri: "/x".into(),
                ..Default::default()
            })
            .point_id()
            .0
        };
        assert_eq!(route(HttpMethod::GET), "app.py#h3#endpoint#GET /x");
        assert_eq!(route(HttpMethod::POST), "app.py#h3#endpoint#POST /x");
    }

    #[test]
    fn protocol_follows_the_code_element() {
        let edge = |protocol, destination_kind, destination: &str| {
            CodeElement::Message(MessageEdge {
                protocol,
                role: MessageRole::Producer,
                destination_kind,
                destination: destination.into(),
                exchange: None,
                routing_key: None,
                queue: None,
                topic: None,
                handler: None,
                function_name: "get".into(),
                function_hash: "h2".into(),
                call_arguments: vec![],
                file_path: "a/client.go".into(),
            })
        };
        use CommunicationProtocol as C;
        use MessageDestinationKind as D;
        assert_eq!(call(0, 0).protocol(), Protocol::Http);
        let endpoint = CodeElement::Endpoint(Endpoint::default());
        assert_eq!(endpoint.protocol(), Protocol::Http);
        let grpc = edge(C::Grpc, D::GrpcService, "orders.Orders/Get");
        assert_eq!(grpc.protocol(), Protocol::Grpc);
        let kafka = edge(C::Kafka, D::Topic, "orders.created");
        assert_eq!(kafka.protocol(), Protocol::Kafka);
        let rabbit = edge(C::RabbitMq, D::Queue, "orders");
        assert_eq!(rabbit.protocol(), Protocol::RabbitMq);
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
