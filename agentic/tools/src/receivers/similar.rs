//! `similar`: closest receivers in other services.

use agentic_models::{Address, ConnectionPoint};
use models::HttpMethod;
use strsim::normalized_levenshtein;

use super::{address_key, row};

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

/// HTTP method; `None` for any other address, so non-HTTP never filters.
fn method(address: &Address) -> Option<Option<HttpMethod>> {
    match address {
        Address::Http { method, .. } => Some(*method),
        _ => None,
    }
}
