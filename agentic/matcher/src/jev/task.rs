//! Task text for the two sessions: the call, its bindings, its candidates.

use agentic_models::{CodeElement, ConnectionPoint};
use agentic_tools::row;
use extractor_runtime::pipeline::pass3::OperandBindings;

use crate::plan::Pending;

/// Longest call text shown; a call with a large inline payload is cut.
const MAX_CODE: usize = 300;

/// Service session task; `code` is the call expression's source text.
pub fn service_task(p: &Pending, bindings: Option<&OperandBindings>, code: Option<&str>) -> String {
    let call = &p.call;
    let (method, raw) = match &call.element {
        CodeElement::RestCall(rc) => (format!("{:?}", rc.http_method), rc.target_uri.as_str()),
        _ => (String::new(), ""),
    };
    let mut lines = vec![format!("call: {method} {raw}")];
    lines.extend(code.map(|c| format!("code: {}", one_line(c))));
    lines.extend([
        format!("substituted: {}", p.target),
        format!(
            "from: {}, {}, {}",
            call.service.0,
            call.element.file(),
            call.element.function_name()
        ),
    ]);
    lines.extend(
        bindings
            .into_iter()
            .flatten()
            .map(|(k, v)| format!("binding: {k} = {v}")),
    );
    lines.extend(candidates(&p.candidates));
    lines.join("\n")
}

/// Endpoint session task: the service task plus the picked service.
pub fn endpoint_task(service_task: &str, service: &str) -> String {
    format!("{service_task}\npicked service: {service}")
}

/// Whitespace runs collapsed to one space, cut at `MAX_CODE` chars.
fn one_line(code: &str) -> String {
    let flat = code.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.char_indices().nth(MAX_CODE) {
        Some((i, _)) => format!("{} ...", &flat[..i]),
        None => flat,
    }
}

fn candidates(found: &[ConnectionPoint]) -> Vec<String> {
    if found.is_empty() {
        return vec!["candidate: none".into()];
    }
    found
        .iter()
        .map(|r| format!("candidate: {}", row(r)))
        .collect()
}
