use super::*;
use models::{CommunicationProtocol, MessageDestinationKind};

fn svc(name: &str, base: &str) -> ServiceDescription {
    ServiceDescription {
        name: name.into(),
        base_dir_path: base.into(),
        urls: vec![format!("http://{name}:8000")],
        java_analysis: None,
    }
}

pub(crate) fn edge(protocol: CommunicationProtocol, role: MessageRole, dest: &str) -> MessageEdge {
    MessageEdge {
        protocol,
        role,
        destination_kind: MessageDestinationKind::Unknown,
        destination: dest.into(),
        exchange: None,
        routing_key: None,
        queue: None,
        topic: None,
        handler: None,
        function_name: "f".into(),
        function_hash: "h".into(),
        call_arguments: vec![],
        file_path: "a/x.java".into(),
    }
}

pub(crate) fn ir(
    endpoints: Vec<Endpoint>,
    restcalls: Vec<RestCall>,
    message_edges: Vec<MessageEdge>,
) -> EvaluatedIR {
    EvaluatedIR {
        endpoints,
        restcalls,
        message_edges,
        ..Default::default()
    }
}

#[test]
fn address_splits_host_and_path() {
    let call = RestCall {
        target_uri: "{MDS}/v3/cases".into(),
        ..Default::default()
    };
    let p = rest_call(call, ServiceId("a".into()), "http://mds:8000/v3/cases");
    let Address::Http { host, path, .. } = &p.address else {
        panic!("http address")
    };
    assert_eq!((host.as_deref(), path.as_str()), (Some("mds"), "/v3/cases"));
    let CodeElement::RestCall(c) = &p.element else {
        panic!("rest call")
    };
    assert_eq!(c.target_uri, "{MDS}/v3/cases");
}

#[test]
fn relative_target_has_no_host() {
    let p = rest_call(RestCall::default(), ServiceId::default(), "/v3/cases");
    let Address::Http { host, path, .. } = &p.address else {
        panic!("http address")
    };
    assert_eq!((host.as_deref(), path.as_str()), (None, "/v3/cases"));
}

#[test]
fn message_addresses() {
    use CommunicationProtocol::{Grpc, RabbitMq};
    let pairs = [("Queues.queueName".to_string(), "orders".to_string())];
    let substituter = Substituter::new(&pairs).expect("regex compiles");
    let addr = |e: MessageEdge| message::address(&e, &substituter);
    let rpc = |service: &str, method: &str| Address::Rpc {
        service: service.into(),
        method: method.into(),
    };
    assert_eq!(
        addr(edge(Grpc, MessageRole::Producer, "Orders/Get")),
        rpc("Orders", "Get")
    );
    assert_eq!(
        addr(edge(Grpc, MessageRole::Producer, "Orders")),
        rpc("Orders", "")
    );
    let mut rabbit = edge(RabbitMq, MessageRole::Producer, "Queues.queueName");
    rabbit.exchange = Some("ex".into());
    assert_eq!(
        addr(rabbit),
        Address::Channel {
            name: "orders".into(),
            broker: Some("ex".into()),
        }
    );
    assert_eq!(
        addr(edge(RabbitMq, MessageRole::Producer, " ")),
        Address::Unresolved { raw: " ".into() }
    );
}

#[test]
fn build_splits_roles() {
    use CommunicationProtocol::RabbitMq;
    let edges = vec![
        edge(RabbitMq, MessageRole::Producer, "q"),
        edge(RabbitMq, MessageRole::Consumer, "q"),
        edge(RabbitMq, MessageRole::Binding, "q"),
    ];
    let mut metrics = Metrics::default();
    let substituter = Substituter::new(&[]).expect("no constants");
    let pts = build(
        &ir(vec![], vec![], edges),
        &[svc("a", "a")],
        &substituter,
        &mut metrics,
    );
    assert_eq!((pts.initiators.len(), pts.receivers.len()), (1, 1));
    assert_eq!(pts.initiators[0].protocol, Protocol::RabbitMq);
    assert_eq!(metrics.of(Protocol::RabbitMq).topology, 1);
}
