//! Closed answer sets: services first, then one service's endpoints.

use std::collections::{BTreeMap, BTreeSet};

use agent_loop::adapters::{AnswerOption, Answers};
use agentic_models::ConnectionPoint;
use agentic_tools::address_key;
use clients::typesafe::question::MAX_CHOICE_OPTIONS;
use models::configuration::ServiceDescription;
use strsim::normalized_levenshtein;

use super::{Pick, Target};
use crate::http::{method, path};

/// Endpoint answers kept; one choice slot stays for `none`.
const MAX_ENDPOINTS: usize = MAX_CHOICE_OPTIONS - 1;

/// Configured services with a receiver of `call`'s protocol, own included,
/// then the two answers for calls that reach no configured service.
pub fn services(
    call: &ConnectionPoint,
    config: &[ServiceDescription],
    receivers: &[&ConnectionPoint],
) -> Answers<Pick> {
    let configured: BTreeSet<&str> = config.iter().map(|sd| sd.name.as_str()).collect();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for r in receivers {
        let name = r.service.0.as_str();
        if r.protocol == call.protocol && configured.contains(name) {
            *counts.entry(name).or_default() += 1;
        }
    }
    let mut options: Vec<AnswerOption<Pick>> = counts
        .into_iter()
        .map(|(name, n)| AnswerOption {
            label: name.to_string(),
            description: format!("{n} {:?} receivers", call.protocol),
            value: Some(Target::Point(name.to_string())),
        })
        .collect();
    if !options.is_empty() {
        options.extend(not_a_service());
    }
    Answers {
        options,
        none: None,
    }
}

/// Receivers of `service` with `call`'s protocol and, when known, method;
/// the closest `MAX_ENDPOINTS` to the call path when there are more.
pub fn endpoints(
    call: &ConnectionPoint,
    service: &str,
    receivers: &[&ConnectionPoint],
) -> Answers<Pick> {
    let mut eps: Vec<&ConnectionPoint> = receivers
        .iter()
        .copied()
        .filter(|r| r.service.0 == service && r.protocol == call.protocol)
        .filter(|r| method(call).is_none() || method(r) == method(call))
        .collect();
    if eps.len() > MAX_ENDPOINTS {
        let closeness = |r: &ConnectionPoint| normalized_levenshtein(path(r), path(call));
        eps.sort_by(|a, b| closeness(b).total_cmp(&closeness(a)));
        eps.truncate(MAX_ENDPOINTS);
    }
    eps.sort_by_key(|r| (address_key(&r.address), r.id.clone()));
    let options = eps
        .iter()
        .enumerate()
        .map(|(i, r)| AnswerOption {
            label: format!("e{}", i + 1),
            description: format!(
                "{} | {}",
                address_key(&r.address),
                r.element.function_name()
            ),
            value: Some(Target::Point(r.id.0.clone())),
        })
        .collect();
    Answers {
        options,
        none: None,
    }
}

/// The fixed answers for calls that reach no configured service.
fn not_a_service() -> [AnswerOption<Pick>; 2] {
    [
        AnswerOption {
            label: "external".into(),
            description: "Calls a system outside the configured services, \
                          e.g. a third-party API, a secret store or a container registry."
                .into(),
            value: Some(Target::External),
        },
        AnswerOption {
            label: "not-a-service".into(),
            description: "Does not call a service's API: an in-memory map or cache, \
                          a database or other data-store client, or a library/SDK object."
                .into(),
            value: Some(Target::NotAService),
        },
    ]
}
