//! `--jev`: Jev sessions for the initiators the static pass left pending.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use agent_loop::Log;
use agent_loop::adapters::{Answers, JevLimits, JevModel, JevWording};
use agentic_matcher::Plan;
use agentic_matcher::jev::{self, JevCtx, JevFactory, JevResult, Pick, Stage};
use agentic_models::ConnectionPoint;
use agentic_tools::ToolCtx;
use anyhow::{Context, Result};
use clients::typesafe::TypeSafeClient;
use models::ConfigurationData;

use crate::extract::Extracted;

/// Builds a `JevModel` per session with that stage's wording.
struct Factory<'a> {
    client: &'a TypeSafeClient,
    service: JevWording,
    endpoint: JevWording,
}

impl JevFactory for Factory<'_> {
    type M<'b>
        = JevModel<'b, Pick>
    where
        Self: 'b;

    fn model(&self, stage: Stage, answers: Answers<Pick>) -> JevModel<'_, Pick> {
        let wording = match stage {
            Stage::Service => &self.service,
            Stage::Endpoint => &self.endpoint,
        };
        JevModel::new(self.client, wording, answers, JevLimits::default())
    }
}

/// Runs every pending session, logging to `out/match/sessions.jsonl`.
pub async fn run(
    client: &TypeSafeClient,
    plan: &Plan,
    extracted: &Extracted,
    config: &ConfigurationData,
    constants: &HashMap<String, String>,
    out: &Path,
) -> Result<JevResult> {
    let dir = out.join("match");
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let log = Log::open(&dir.join("sessions.jsonl")).context("opening session log")?;
    let receivers: Vec<&ConnectionPoint> = plan.receivers.iter().collect();
    let ctx = JevCtx {
        tools: ToolCtx {
            project: &extracted.project,
            config,
            constants,
            receivers: &receivers,
        },
        bindings: &extracted.bindings,
    };
    let factory = Factory {
        client,
        service: Stage::Service.wording(),
        endpoint: Stage::Endpoint.wording(),
    };
    Ok(jev::resolve(&plan.pending, &ctx, &factory, &log).await?)
}
