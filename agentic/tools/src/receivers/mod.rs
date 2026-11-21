//! Receiver facts: receivers the initiator could reach, one tool per file.

mod service;
mod similar;

pub use service::of_service;
pub use similar::similar;

use agentic_models::{Address, ConnectionPoint};

/// `GET /v3/cases`, `svc/Method`, `broker:name`.
pub fn address_key(address: &Address) -> String {
    match address {
        Address::Http { method, path, .. } => match method {
            Some(m) => format!("{m:?} {path}"),
            None => path.clone(),
        },
        Address::Rpc { service, method } => format!("{service}/{method}"),
        Address::Channel { name, broker } => match broker {
            Some(b) => format!("{b}:{name}"),
            None => name.clone(),
        },
        Address::Resource { name } => name.clone(),
        Address::Unresolved { raw } => raw.clone(),
    }
}

/// `<point id> | <service> | <address>`.
pub fn row(r: &ConnectionPoint) -> String {
    format!("{} | {} | {}", r.id.0, r.service.0, address_key(&r.address))
}
