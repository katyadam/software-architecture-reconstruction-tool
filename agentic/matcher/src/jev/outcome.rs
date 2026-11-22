//! Two sessions -> one link or one `Unmatched`, with both sessions' facts.

use agent_loop::{Answer, Outcome, Session};
use agentic_models::{
    ConnectionPoint, Excluded, ExcludedReason, Fact, FactId, InteractionKind, Link, MatchSource,
    ServiceId, Unmatched, UnmatchedReason,
};

use super::{Pick, Target};
use crate::http::request_kind;
use crate::plan::Pending;
use crate::services::Services;

#[derive(Debug)]
pub enum Resolved {
    Link(ServiceId, Box<Link>),
    Unmatched(Box<Unmatched>),
    Excluded(Box<Excluded>),
}

/// The point's sessions and what they can pick from.
pub struct Sessions<'a> {
    pub service: &'a Session<Pick>,
    /// `None` when no endpoint session ran.
    pub endpoint: Option<&'a Session<Pick>>,
    pub receivers: &'a [&'a ConnectionPoint],
}

/// Maps the sessions of `p` per the spec's outcome table.
pub fn resolve(p: &Pending, s: &Sessions, services: &Services) -> Resolved {
    let evidence = renumber(s);
    let unmatched = |reason, note| self::unmatched(p, reason, note, evidence.clone());
    let (service, service_conf) = match &s.service.outcome {
        Outcome::Answer(Answer {
            value: Some(target),
            confidence,
            ..
        }) => match target {
            Target::Point(name) => (name, *confidence),
            Target::External => {
                return excluded(p, ExcludedReason::External, *confidence, evidence);
            }
            Target::InProcess => {
                return excluded(p, ExcludedReason::InProcess, *confidence, evidence);
            }
        },
        Outcome::Answer(a) => {
            return unmatched(UnmatchedReason::NoFit, a.note.clone().unwrap_or_default());
        }
        Outcome::BudgetExhausted => {
            return unmatched(UnmatchedReason::BudgetExhausted, String::new());
        }
        Outcome::ModelError(e) => return unmatched(UnmatchedReason::ModelError, e.clone()),
    };
    let (receiver, confidence) = match s.endpoint.map(|e| &e.outcome) {
        Some(Outcome::ModelError(e)) => return unmatched(UnmatchedReason::ModelError, e.clone()),
        Some(Outcome::Answer(Answer {
            value: Some(Target::Point(id)),
            confidence,
            ..
        })) => match s.receivers.iter().find(|r| r.id.0 == *id) {
            Some(r) => (Some(*r), *confidence),
            None => (None, service_conf),
        },
        _ => (None, service_conf),
    };
    let link = Link {
        initiator: p.call.clone(),
        receiver: receiver.cloned(),
        // A self-call is reflexive whatever its URL says.
        kind: match *service == p.call.service.0 {
            true => InteractionKind::Reflexive,
            false => request_kind(&p.call, &p.target, receiver, services),
        },
        match_source: MatchSource::Jev,
        confidence,
        evidence,
    };
    Resolved::Link(ServiceId(service.clone()), Box::new(link))
}

/// `p` unmatched; `tried` holds its static candidates.
pub fn unmatched(
    p: &Pending,
    reason: UnmatchedReason,
    note: String,
    evidence: Vec<Fact>,
) -> Resolved {
    Resolved::Unmatched(Box::new(Unmatched {
        point: p.call.clone(),
        reason,
        tried: p.candidates.iter().map(|r| r.id.clone()).collect(),
        banned: vec![],
        note,
        evidence,
    }))
}

/// `p` left out of the SDG; the note keeps the answer's confidence and the
/// static candidates, so an audit sees receivers the static rules found.
fn excluded(
    p: &Pending,
    reason: ExcludedReason,
    confidence: Option<f64>,
    evidence: Vec<Fact>,
) -> Resolved {
    let mut note = confidence.map_or(String::new(), |c| format!("confidence {c}"));
    if !p.candidates.is_empty() {
        let ids: Vec<&str> = p.candidates.iter().map(|r| r.id.0.as_str()).collect();
        note.push_str(&format!("; candidates: {}", ids.join(", ")));
    }
    Resolved::Excluded(Box::new(Excluded {
        point: p.call.clone(),
        reason,
        note,
        evidence,
    }))
}

/// Service facts, then endpoint facts, numbered `f1..`.
fn renumber(s: &Sessions) -> Vec<Fact> {
    s.service
        .facts
        .iter()
        .chain(s.endpoint.into_iter().flat_map(|e| &e.facts))
        .enumerate()
        .map(|(i, f)| Fact {
            id: FactId(format!("f{}", i + 1)),
            ..f.clone()
        })
        .collect()
}
