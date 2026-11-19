//! Jev adapter: one System One request per step. Decision rule ported from
//! DDFA (`extractor-runtime/.../llm_enhance/ddfa.rs`).

pub(super) mod questions;
pub(super) mod reply;

use std::fmt::Debug;

use agentic_models::Fact;
use clients::typesafe::question::NoulCriteria;
use clients::typesafe::{JEV_MODEL, SystemOneRequest, TypeSafeClient};
use serde::Serialize;

use super::{Answers, fingerprint};
use crate::protocol::{Model, Reply, Request};

use questions::questions;
use reply::step_from;

pub(super) const ANSWER: &str = "answer";
pub(super) const ENOUGH: &str = "enough";
pub(super) const NEXT: &str = "next";
pub(super) const NONE: &str = "none";
pub(super) const STOP: &str = "stop";

/// Question texts; stage-specific, so no default.
#[derive(Debug, Clone)]
pub struct JevWording {
    pub answer: String,
    pub enough: String,
    pub enough_criteria: NoulCriteria,
    pub next: String,
}

#[derive(Debug, Clone, Copy)]
pub struct JevLimits {
    /// `enough` at or above this -> answer.
    pub enough: f64,
    /// Answer probability below this -> `none`.
    pub answer_min: f64,
}

impl Default for JevLimits {
    fn default() -> Self {
        Self {
            enough: 0.7,
            answer_min: 0.5,
        }
    }
}

#[derive(Serialize)]
struct JevState<'a> {
    task: &'a str,
    facts: &'a [Fact],
}

/// Built per session, with that session's answers.
pub struct JevModel<'a, A> {
    client: &'a TypeSafeClient,
    wording: &'a JevWording,
    answers: Answers<A>,
    limits: JevLimits,
    /// Jev version plus a hash of wording, limits and answers, so editing
    /// any of them changes the prompt hash and skips stale log replays.
    id: String,
}

impl<'a, A: Debug> JevModel<'a, A> {
    pub fn new(
        client: &'a TypeSafeClient,
        wording: &'a JevWording,
        answers: Answers<A>,
        limits: JevLimits,
    ) -> Self {
        Self {
            id: model_id(wording, limits, &answers),
            client,
            wording,
            answers,
            limits,
        }
    }
}

pub(super) fn model_id<A: Debug>(
    wording: &JevWording,
    limits: JevLimits,
    answers: &Answers<A>,
) -> String {
    fingerprint(JEV_MODEL, &format!("{wording:?}{limits:?}{answers:?}"))
}

impl<A: Clone + Send + Sync> Model<A> for JevModel<'_, A> {
    fn id(&self) -> &str {
        &self.id
    }

    async fn step(&mut self, req: &Request<'_>) -> Result<Reply<A>, String> {
        let questions = questions(self.wording, &self.answers, req.options)?;
        let state = JevState {
            task: req.task,
            facts: req.facts,
        };
        let response = self
            .client
            .system_one(&SystemOneRequest::new(state, questions))
            .await
            .map_err(|e| format!("jev: {e}"))?;
        let offered = !req.options.is_empty();
        Ok(Reply {
            step: step_from(&response, &self.answers, self.limits, offered)?,
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
        })
    }
}
