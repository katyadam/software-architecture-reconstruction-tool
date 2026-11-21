use std::collections::{BTreeMap, BTreeSet};

use agent_loop::Tools;
use agentic_models::{Address, CodeElement, ConnectionPoint, Protocol, ServiceId, Side};
use extractor_runtime::pipeline::facts::test_fixtures::{ORIGIN, World, file, world};
use extractor_runtime::pipeline::pass3::OperandBindings;
use models::ir::language::Language;
use models::{
    CommunicationProtocol, Endpoint, HttpMethod, MessageDestinationKind, MessageEdge, MessageRole,
    RestCall, source_code::SourceSpan,
};

use super::{PointTools, ToolCtx};

fn point(side: Side, service: &str, element: CodeElement, address: Address) -> ConnectionPoint {
    ConnectionPoint {
        id: element.point_id(),
        service: ServiceId(service.into()),
        side,
        protocol: element.protocol(),
        address,
        element,
    }
}

fn http(method: HttpMethod, path: &str) -> Address {
    Address::Http {
        method: Some(method),
        host: None,
        path: path.into(),
    }
}

/// The fixture residual as an initiator in `caller`.
fn call(method: HttpMethod, path: &str) -> ConnectionPoint {
    let rc = RestCall {
        function_name: "fetch_case".into(),
        function_hash: "h1".into(),
        call_arguments: vec![],
        http_method: method,
        target_uri: "self._mds_url + case_id".into(),
        file_path: ORIGIN.into(),
        source_span: SourceSpan::new(0, 0),
    };
    point(
        Side::Initiator,
        "caller",
        CodeElement::RestCall(rc),
        http(method, path),
    )
}

fn ep(service: &str, method: HttpMethod, uri: &str) -> ConnectionPoint {
    let e = Endpoint {
        http_method: method,
        uri: uri.into(),
        file_path: format!("/proj/{service}/{}.py", uri.replace('/', "_")),
        ..Default::default()
    };
    point(
        Side::Receiver,
        service,
        CodeElement::Endpoint(e),
        http(method, uri),
    )
}

fn edge(role: MessageRole, destination: &str, file: &str) -> MessageEdge {
    MessageEdge {
        protocol: CommunicationProtocol::RabbitMq,
        role,
        destination_kind: MessageDestinationKind::Queue,
        destination: destination.into(),
        exchange: None,
        routing_key: None,
        queue: None,
        topic: None,
        handler: None,
        function_name: "send".into(),
        function_hash: "h7".into(),
        call_arguments: vec![],
        file_path: file.into(),
    }
}

fn channel(name: &str) -> Address {
    Address::Channel {
        name: name.into(),
        broker: None,
    }
}

fn bindings() -> OperandBindings {
    BTreeMap::from([
        ("self._mds_url".to_string(), "settings.mds_url".to_string()),
        ("case_id".to_string(), "unresolved: str".to_string()),
    ])
}

fn ids(tools: &PointTools, taken: &BTreeSet<String>) -> Vec<String> {
    tools.available(taken).into_iter().map(|o| o.id).collect()
}

/// Runs the offered option `id`.
fn run(tools: &PointTools, id: &str) -> String {
    let option = tools
        .available(&BTreeSet::new())
        .into_iter()
        .find(|o| o.id == id)
        .unwrap_or_else(|| panic!("{id} not offered"));
    tools.run(&option)
}

fn ctx<'a>(w: &'a World, receivers: &'a [&'a ConnectionPoint]) -> ToolCtx<'a> {
    ToolCtx {
        project: &w.ir,
        config: &w.config,
        constants: &w.env,
        receivers,
    }
}

#[test]
fn options_in_spec_order_with_ddfa_code_facts() {
    let (w, c) = (world(), call(HttpMethod::GET, "/cases"));
    let (own, mds) = (
        ep("caller", HttpMethod::POST, "/x"),
        ep("mds", HttpMethod::GET, "/y"),
    );
    let receivers = [&own, &mds];
    let ctx = ctx(&w, &receivers);
    let b = bindings();
    let tools = PointTools::new(&ctx, &c, Some(&b)).expect("tools");
    // `auth` has no HTTP receiver, so no `service:auth`.
    let expected = [
        "define:mds_url",
        "constant:mds_url",
        "callers",
        "receiver",
        "open:proj/caller/settings.py#settings",
        "similar",
        "imports",
        "service:caller",
        "service:mds",
    ];
    assert_eq!(ids(&tools, &BTreeSet::new()), expected);
}

#[test]
fn taken_option_not_offered() {
    let (w, c) = (world(), call(HttpMethod::GET, "/cases"));
    let ctx = ctx(&w, &[]);
    let tools = PointTools::new(&ctx, &c, None).expect("tools");
    let taken = BTreeSet::from(["similar".to_string()]);
    assert!(!ids(&tools, &taken).contains(&"similar".to_string()));
}

#[test]
fn opened_file_widens_options() {
    let (w, c) = (world(), call(HttpMethod::GET, "/cases"));
    let ctx = ctx(&w, &[]);
    let b = bindings();
    let tools = PointTools::new(&ctx, &c, Some(&b)).expect("tools");
    run(&tools, "open:proj/caller/settings.py#settings");
    let offered = ids(&tools, &BTreeSet::new());
    assert!(offered.contains(&"open:proj/caller/config.py#config".to_string()));
}

#[test]
fn similar_other_service_only() {
    let w = world();
    let c = call(HttpMethod::GET, "/v3/cases");
    let (own, mds) = (
        ep("caller", HttpMethod::GET, "/v3/cases"),
        ep("mds", HttpMethod::GET, "/v3/cases"),
    );
    let receivers = [&own, &mds];
    let ctx = ctx(&w, &receivers);
    let tools = PointTools::new(&ctx, &c, None).expect("tools");
    let expected = format!("{} | mds | GET /v3/cases", mds.id.0);
    assert_eq!(run(&tools, "similar"), expected);
}

#[test]
fn similar_filters_method() {
    let w = world();
    let c = call(HttpMethod::GET, "/v3/cases");
    let post = ep("mds", HttpMethod::POST, "/v3/cases");
    let receivers = [&post];
    let ctx = ctx(&w, &receivers);
    let tools = PointTools::new(&ctx, &c, None).expect("tools");
    assert_eq!(run(&tools, "similar"), "(nothing found)");
}

#[test]
fn service_lists_any_method_capped() {
    let w = world();
    let c = call(HttpMethod::GET, "/x");
    let eps: Vec<ConnectionPoint> = (0..55)
        .map(|i| ep("mds", HttpMethod::POST, &format!("/r{i:02}")))
        .collect();
    let receivers: Vec<&ConnectionPoint> = eps.iter().collect();
    let ctx = ctx(&w, &receivers);
    let tools = PointTools::new(&ctx, &c, None).expect("tools");
    let out = run(&tools, "service:mds");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 41);
    assert!(lines[0].ends_with("| mds | POST /r00"));
    assert_eq!(lines[40], "... 15 more");
    assert!(!ids(&tools, &BTreeSet::new()).contains(&"service:auth".to_string()));
}

#[test]
fn imports_both_directions() {
    let mut w = world();
    // Another language importing the origin stays out.
    w.link("/proj/caller/Stub.java", "client", ORIGIN);
    let c = call(HttpMethod::GET, "/x");
    let ctx = ctx(&w, &[]);
    let tools = PointTools::new(&ctx, &c, None).expect("tools");
    let out = run(&tools, "imports");
    assert!(
        out.contains("imports settings from proj/caller/settings.py"),
        "{out}"
    );
    assert!(
        out.contains("imported by proj/caller/settings.py as client"),
        "{out}"
    );
    assert!(!out.contains("Stub.java as client"), "{out}");
}

#[test]
fn message_producer_tools() {
    let sender = "/proj/caller/sender.py";
    let mut w = world();
    w.ir.files.push(file(sender, Language::Python));
    let producer = edge(MessageRole::Producer, "Queues.queueName", sender);
    let p = point(
        Side::Initiator,
        "caller",
        CodeElement::Message(producer),
        channel("Queues.queueName"),
    );
    let consumer = edge(MessageRole::Consumer, "orders", "/proj/mds/c.py");
    let rabbit = point(
        Side::Receiver,
        "mds",
        CodeElement::Message(consumer),
        channel("orders"),
    );
    let http_ep = ep("mds", HttpMethod::GET, "/orders");
    let receivers = [&rabbit, &http_ep];
    let ctx = ctx(&w, &receivers);
    let tools = PointTools::new(&ctx, &p, None).expect("tools");
    assert_eq!(p.protocol, Protocol::RabbitMq);
    assert!(ids(&tools, &BTreeSet::new()).contains(&"define:queueName".to_string()));
    let expected = format!("{} | mds | orders", rabbit.id.0);
    assert_eq!(run(&tools, "similar"), expected);
    assert_eq!(run(&tools, "service:mds"), expected);
}

#[test]
fn receiver_or_missing_file_has_no_tools() {
    let w = world();
    let e = ep("mds", HttpMethod::GET, "/x");
    let ctx = ctx(&w, &[]);
    assert!(PointTools::new(&ctx, &e, None).is_none());
    let consumer = edge(MessageRole::Consumer, "orders", ORIGIN);
    let consumer = point(
        Side::Receiver,
        "caller",
        CodeElement::Message(consumer),
        channel("orders"),
    );
    assert!(PointTools::new(&ctx, &consumer, None).is_none());
    let mut c = call(HttpMethod::GET, "/x");
    if let CodeElement::RestCall(rc) = &mut c.element {
        rc.file_path = "/elsewhere/a.py".into();
    }
    assert!(PointTools::new(&ctx, &c, None).is_none());
}
