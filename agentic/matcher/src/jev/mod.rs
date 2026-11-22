//! Jev stage: for each pending initiator, a service session, then an
//! endpoint session within the picked service.

mod answers;
mod outcome;
mod task;
mod wording;

use std::collections::HashMap;
use std::io;

use agent_loop::{Log, Outcome, Session, SessionKey, run_session};
use agentic_models::{Excluded, Link, PointId, ServiceId, Unmatched, UnmatchedReason};
use agentic_tools::{PointTools, ToolCtx};
use extractor_runtime::pipeline::pass3::OperandBindings;

use crate::metrics::JevMetrics;
use crate::plan::Pending;
use crate::services::Services;
use outcome::{Resolved, Sessions};

pub use wording::{JevFactory, Pick, Stage, Target};

/// Tool calls per session.
const BUDGET: usize = 6;

pub struct JevCtx<'a> {
    pub tools: ToolCtx<'a>,
    pub bindings: &'a HashMap<PointId, OperandBindings>,
}

#[derive(Debug, Default)]
pub struct JevResult {
    pub links: Vec<(ServiceId, Link)>,
    pub unmatched: Vec<Unmatched>,
    pub excluded: Vec<Excluded>,
    pub metrics: JevMetrics,
}

/// Runs the sessions of every pending point, in the given order.
pub async fn resolve<F: JevFactory>(
    pending: &[Pending],
    ctx: &JevCtx<'_>,
    factory: &F,
    log: &Log,
) -> io::Result<JevResult> {
    let services = Services::new(&ctx.tools.config.service_descriptions);
    let mut out = JevResult::default();
    for p in pending {
        out.metrics.initiators += 1;
        let tools = PointTools::new(&ctx.tools, &p.call, ctx.bindings.get(&p.call.id));
        let resolved = match tools {
            Some(tools) => {
                let run = Run {
                    p,
                    tools: &tools,
                    factory,
                    log,
                };
                one(&run, ctx, &services, &mut out.metrics).await?
            }
            // Never expected: pending calls come from the extracted project.
            None => no_candidate(p, "no tools: file not in project"),
        };
        out.metrics.count(&resolved);
        match resolved {
            Resolved::Link(target, link) => out.links.push((target, *link)),
            Resolved::Unmatched(u) => out.unmatched.push(*u),
            Resolved::Excluded(e) => out.excluded.push(*e),
        }
    }
    Ok(out)
}

/// The service session, then the endpoint session when a service with
/// matching receivers was picked.
async fn one<F: JevFactory>(
    run: &Run<'_, F>,
    ctx: &JevCtx<'_>,
    services: &Services<'_>,
    metrics: &mut JevMetrics,
) -> io::Result<Resolved> {
    let p = run.p;
    let receivers = ctx.tools.receivers;
    let task = task::service_task(p, ctx.bindings.get(&p.call.id), run.tools.call());
    let answers = answers::services(&p.call, &ctx.tools.config.service_descriptions, receivers);
    if answers.options.is_empty() {
        let note = format!("no service has {:?} receivers", p.call.protocol);
        return Ok(no_candidate(p, &note));
    }
    let service = run.session(Stage::Service, &task, answers, metrics).await?;
    let mut endpoint = None;
    if let Outcome::Answer(a) = &service.outcome
        && let Some(Target::Point(name)) = &a.value
    {
        let answers = answers::endpoints(&p.call, name, receivers);
        if !answers.options.is_empty() {
            let task = task::endpoint_task(&task, name);
            endpoint = Some(
                run.session(Stage::Endpoint, &task, answers, metrics)
                    .await?,
            );
        }
    }
    let sessions = Sessions {
        service: &service,
        endpoint: endpoint.as_ref(),
        receivers,
    };
    Ok(outcome::resolve(p, &sessions, services))
}

/// What every session of one point shares.
struct Run<'a, F> {
    p: &'a Pending,
    tools: &'a PointTools<'a>,
    factory: &'a F,
    log: &'a Log,
}

impl<F: JevFactory> Run<'_, F> {
    async fn session(
        &self,
        stage: Stage,
        task: &str,
        answers: agent_loop::adapters::Answers<Pick>,
        metrics: &mut JevMetrics,
    ) -> io::Result<Session<Pick>> {
        let key = SessionKey {
            stage: stage.name().into(),
            subject: self.p.call.id.0.clone(),
        };
        let mut model = self.factory.model(stage, answers);
        let s = run_session(&key, task, &mut model, self.tools, BUDGET, self.log).await?;
        metrics.session(&s);
        Ok(s)
    }
}

/// `NoCandidate` with no session run.
fn no_candidate(p: &Pending, note: &str) -> Resolved {
    outcome::unmatched(p, UnmatchedReason::NoCandidate, note.into(), vec![])
}

impl JevMetrics {
    fn session<A>(&mut self, s: &Session<A>) {
        self.sessions += 1;
        self.replayed += usize::from(s.replayed);
        self.model_calls += s.calls;
        self.input_tokens += s.input_tokens;
        self.output_tokens += s.output_tokens;
    }

    fn count(&mut self, resolved: &Resolved) {
        match resolved {
            Resolved::Link(_, l) if l.receiver.is_some() => self.receiver_links += 1,
            Resolved::Link(..) => self.service_links += 1,
            Resolved::Unmatched(u) => {
                *self.unmatched.entry(format!("{:?}", u.reason)).or_default() += 1
            }
            Resolved::Excluded(e) => {
                *self.excluded.entry(format!("{:?}", e.reason)).or_default() += 1
            }
        }
    }
}

#[cfg(test)]
mod tests;
