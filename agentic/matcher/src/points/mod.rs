//! Connection points with their services, from every extracted element.

mod message;

use agentic_models::{Address, CodeElement, ConnectionPoint, Protocol, ServiceId, Side};
use models::configuration::{ServiceDescription, assign_service};
use models::interaction_kind::host_of;
use models::ir::evaluted::EvaluatedIR;
use models::substitute::Substituter;
use models::{Endpoint, MessageEdge, MessageRole, RestCall};

use crate::metrics::Metrics;

/// A connection point with the service it belongs to.
#[derive(Debug, Clone)]
pub struct Point {
    pub point: ConnectionPoint,
    pub service: ServiceDescription,
}

impl Point {
    fn new(
        side: Side,
        element: CodeElement,
        service: ServiceDescription,
        address: Address,
    ) -> Self {
        let point = ConnectionPoint {
            id: element.point_id(),
            service: ServiceId(service.name.clone()),
            side,
            protocol: element.protocol(),
            address,
            element,
        };
        Self { point, service }
    }

    pub fn function_name(&self) -> &str {
        match &self.point.element {
            CodeElement::RestCall(c) => &c.function_name,
            CodeElement::Endpoint(e) => &e.function_name,
            CodeElement::Message(m) => &m.function_name,
        }
    }

    pub fn file(&self) -> &str {
        match &self.point.element {
            CodeElement::RestCall(c) => &c.file_path,
            CodeElement::Endpoint(e) => &e.file_path,
            CodeElement::Message(m) => &m.file_path,
        }
    }
}

/// `target` is the call target after constant substitution.
pub fn rest_call(call: RestCall, service: ServiceDescription, target: &str) -> Point {
    let host = Some(host_of(target).to_string()).filter(|h| !h.is_empty());
    let address = Address::Http {
        method: Some(call.http_method),
        host,
        path: path_of(target).to_string(),
    };
    Point::new(
        Side::Initiator,
        CodeElement::RestCall(call),
        service,
        address,
    )
}

pub fn endpoint(endpoint: Endpoint, service: ServiceDescription) -> Point {
    let address = Address::Http {
        method: Some(endpoint.http_method),
        host: None,
        path: endpoint.uri.clone(),
    };
    Point::new(
        Side::Receiver,
        CodeElement::Endpoint(endpoint),
        service,
        address,
    )
}

/// Producers initiate, consumers receive; `None` for topology roles
/// (bindings, declarations), which are no points.
fn side_of(role: &MessageRole) -> Option<Side> {
    match role {
        MessageRole::Producer => Some(Side::Initiator),
        MessageRole::Consumer => Some(Side::Receiver),
        _ => None,
    }
}

/// The destination gets constant substitution, as REST targets do.
pub fn message(
    edge: MessageEdge,
    side: Side,
    service: ServiceDescription,
    substituter: &Substituter,
) -> Point {
    let address = message::address(&edge, substituter);
    Point::new(side, CodeElement::Message(edge), service, address)
}

#[derive(Debug, Default)]
pub struct Points {
    pub initiators: Vec<Point>,
    pub receivers: Vec<Point>,
}

/// Every element of `ir` as a point. Unassigned initiators are dropped,
/// unassigned receivers stay under the empty service; both are counted.
pub fn build(
    ir: &EvaluatedIR,
    services: &[ServiceDescription],
    substituter: &Substituter,
    metrics: &mut Metrics,
) -> Points {
    let mut out = Points::default();
    for c in &ir.restcalls {
        if let Some(sd) = assigned(
            &c.file_path,
            Protocol::Http,
            Side::Initiator,
            services,
            metrics,
        ) {
            let target = substituter.apply(&c.target_uri);
            out.initiators
                .push(rest_call(c.clone(), sd.clone(), &target));
        }
    }
    for e in &ir.endpoints {
        let sd = assigned(
            &e.file_path,
            Protocol::Http,
            Side::Receiver,
            services,
            metrics,
        );
        out.receivers
            .push(endpoint(e.clone(), sd.cloned().unwrap_or_default()));
    }
    for m in &ir.message_edges {
        add_message(m, services, substituter, metrics, &mut out);
    }
    out
}

fn add_message(
    m: &MessageEdge,
    services: &[ServiceDescription],
    substituter: &Substituter,
    metrics: &mut Metrics,
    out: &mut Points,
) {
    let protocol = Protocol::from(&m.protocol);
    let Some(side) = side_of(&m.role) else {
        metrics.of(protocol).topology += 1;
        return;
    };
    let sd = assigned(&m.file_path, protocol, side, services, metrics);
    match (side, sd) {
        (Side::Initiator, Some(sd)) => {
            let p = message(m.clone(), side, sd.clone(), substituter);
            out.initiators.push(p);
        }
        (Side::Initiator, None) => {}
        (Side::Receiver, sd) => {
            let p = message(
                m.clone(),
                side,
                sd.cloned().unwrap_or_default(),
                substituter,
            );
            out.receivers.push(p);
        }
    }
}

/// The file's service; when there is none, logs it and counts it per protocol.
fn assigned<'s>(
    file: &str,
    protocol: Protocol,
    side: Side,
    services: &'s [ServiceDescription],
    metrics: &mut Metrics,
) -> Option<&'s ServiceDescription> {
    let found = assign_service(file, services);
    if found.is_none() {
        eprintln!("[matcher] unassigned: {file}");
        let m = metrics.of(protocol);
        match side {
            Side::Initiator => m.unassigned_initiators += 1,
            Side::Receiver => m.unassigned_receivers += 1,
        }
    }
    found
}

/// `target` without scheme and authority; a relative target is kept whole.
fn path_of(target: &str) -> &str {
    match target.split_once("://") {
        Some((_, rest)) => rest.find('/').map_or("", |i| &rest[i..]),
        None => target,
    }
}

#[cfg(test)]
pub(crate) mod tests;
