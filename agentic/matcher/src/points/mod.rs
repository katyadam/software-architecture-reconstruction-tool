//! Connection points from every extracted element, with their service ids.

mod message;

use agentic_models::{Address, CodeElement, ConnectionPoint, Protocol, ServiceId, Side};
use models::configuration::{ServiceDescription, assign_service};
use models::interaction_kind::host_of;
use models::ir::evaluted::EvaluatedIR;
use models::substitute::Substituter;
use models::{Endpoint, MessageEdge, MessageRole, RestCall};

use crate::metrics::Metrics;

fn point(
    side: Side,
    element: CodeElement,
    service: ServiceId,
    address: Address,
) -> ConnectionPoint {
    ConnectionPoint {
        id: element.point_id(),
        service,
        side,
        protocol: element.protocol(),
        address,
        element,
    }
}

/// `target` is the call target after constant substitution.
pub fn rest_call(call: RestCall, service: ServiceId, target: &str) -> ConnectionPoint {
    let host = Some(host_of(target).to_string()).filter(|h| !h.is_empty());
    let address = Address::Http {
        method: Some(call.http_method),
        host,
        path: path_of(target).to_string(),
    };
    point(
        Side::Initiator,
        CodeElement::RestCall(call),
        service,
        address,
    )
}

pub fn endpoint(endpoint: Endpoint, service: ServiceId) -> ConnectionPoint {
    let address = Address::Http {
        method: Some(endpoint.http_method),
        host: None,
        path: endpoint.uri.clone(),
    };
    point(
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
    service: ServiceId,
    substituter: &Substituter,
) -> ConnectionPoint {
    let address = message::address(&edge, substituter);
    point(side, CodeElement::Message(edge), service, address)
}

#[derive(Debug, Default)]
pub struct Points {
    pub initiators: Vec<ConnectionPoint>,
    pub receivers: Vec<ConnectionPoint>,
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
        let found = assigned(
            &c.file_path,
            Protocol::Http,
            Side::Initiator,
            services,
            metrics,
        );
        if let Some(id) = found {
            let target = substituter.apply(&c.target_uri);
            out.initiators.push(rest_call(c.clone(), id, &target));
        }
    }
    for e in &ir.endpoints {
        let found = assigned(
            &e.file_path,
            Protocol::Http,
            Side::Receiver,
            services,
            metrics,
        );
        out.receivers
            .push(endpoint(e.clone(), found.unwrap_or_default()));
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
    match (
        side,
        assigned(&m.file_path, protocol, side, services, metrics),
    ) {
        (Side::Initiator, Some(id)) => {
            out.initiators
                .push(message(m.clone(), side, id, substituter))
        }
        (Side::Initiator, None) => {}
        (Side::Receiver, id) => {
            let p = message(m.clone(), side, id.unwrap_or_default(), substituter);
            out.receivers.push(p);
        }
    }
}

/// The file's service; when there is none, logs it and counts it per protocol.
fn assigned(
    file: &str,
    protocol: Protocol,
    side: Side,
    services: &[ServiceDescription],
    metrics: &mut Metrics,
) -> Option<ServiceId> {
    let found = assign_service(file, services);
    if found.is_none() {
        eprintln!("[matcher] unassigned: {file}");
        let m = metrics.of(protocol);
        match side {
            Side::Initiator => m.unassigned_initiators += 1,
            Side::Receiver => m.unassigned_receivers += 1,
        }
    }
    found.map(|sd| ServiceId(sd.name.clone()))
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
