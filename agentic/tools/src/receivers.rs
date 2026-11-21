//! `similar` and `service:<name>`: receivers the initiator could reach.

use agentic_models::{Address, ConnectionPoint};
use models::HttpMethod;
use strsim::normalized_levenshtein;

/// Max rows of `similar`.
const SIMILAR: usize = 10;

/// Other service, same protocol and, for HTTP, same method; closest address first.
pub fn similar(point: &ConnectionPoint, receivers: &[&ConnectionPoint]) -> Vec<String> {
    let key = address_key(&point.address);
    let mut found: Vec<(f64, &ConnectionPoint)> = receivers
        .iter()
        .copied()
        .filter(|r| r.protocol == point.protocol && r.service != point.service)
        .filter(|r| method(&r.address) == method(&point.address))
        .map(|r| {
            (
                1.0 - normalized_levenshtein(&key, &address_key(&r.address)),
                r,
            )
        })
        .collect();
    found.sort_by(|(da, a), (db, b)| {
        da.total_cmp(db)
            .then_with(|| a.service.cmp(&b.service))
            .then_with(|| a.id.cmp(&b.id))
    });
    found
        .into_iter()
        .take(SIMILAR)
        .map(|(_, r)| row(r))
        .collect()
}

/// Every receiver of service `name` with the initiator's protocol, any method.
pub fn of_service(
    point: &ConnectionPoint,
    receivers: &[&ConnectionPoint],
    name: &str,
) -> Vec<String> {
    let mut found: Vec<&ConnectionPoint> = receivers
        .iter()
        .copied()
        .filter(|r| r.protocol == point.protocol && r.service.0 == name)
        .collect();
    found.sort_by_key(|r| (address_key(&r.address), r.id.clone()));
    found.into_iter().map(row).collect()
}

/// HTTP method; `None` for any other address, so non-HTTP never filters.
fn method(address: &Address) -> Option<Option<HttpMethod>> {
    match address {
        Address::Http { method, .. } => Some(*method),
        _ => None,
    }
}

/// `GET /v3/cases`, `svc/Method`, `broker:name`.
fn address_key(address: &Address) -> String {
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
fn row(r: &ConnectionPoint) -> String {
    format!("{} | {} | {}", r.id.0, r.service.0, address_key(&r.address))
}
