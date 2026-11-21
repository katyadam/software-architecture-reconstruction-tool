//! `service:<name>`: every receiver of one service.

use agentic_models::ConnectionPoint;

use super::{address_key, row};

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
