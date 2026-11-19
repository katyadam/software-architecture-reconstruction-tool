//! System-two adapter: any OpenAI-compatible endpoint, one JSON pick per step.

pub(super) mod reply;
pub(super) mod request;

use std::fmt::Debug;
use std::time::Duration;

use async_openai::Client;
use async_openai::config::OpenAIConfig;
use serde_json::Value;

use super::{Answers, fingerprint};
use crate::protocol::{Model, Reply, Request};

use reply::reply_from;
use request::{SYSTEM, request};

const TIMEOUT: Duration = Duration::from_secs(120);
/// Pick prefixes keep a tool id and an equal answer label apart.
const CALL: &str = "call:";
const ANSWER: &str = "answer:";
pub(super) const NONE: &str = "none";

/// Shared across sessions. Key from `LLM_API_KEY`, `ollama` when unset.
pub fn system_two_client(base_url: &str) -> Result<Client<OpenAIConfig>, String> {
    let key = std::env::var("LLM_API_KEY").unwrap_or_else(|_| "ollama".to_string());
    let http = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("system two: {e}"))?;
    let config = OpenAIConfig::new()
        .with_api_base(base_url)
        .with_api_key(key);
    Ok(Client::with_config(config).with_http_client(http))
}

/// Built per session, with that session's answers.
pub struct OpenAiModel<'a, A> {
    client: &'a Client<OpenAIConfig>,
    model: String,
    answers: Answers<A>,
    /// Server-side context size; `None` -> no overflow check.
    context_size: Option<u32>,
    /// Model name plus a hash of the system prompt and answers, so editing
    /// either changes the prompt hash and skips stale log replays.
    id: String,
}

impl<'a, A: Debug> OpenAiModel<'a, A> {
    pub fn new(
        client: &'a Client<OpenAIConfig>,
        model: &str,
        answers: Answers<A>,
        context_size: Option<u32>,
    ) -> Self {
        Self {
            id: fingerprint(model, &format!("{SYSTEM}{answers:?}")),
            client,
            model: model.to_string(),
            answers,
            context_size,
        }
    }
}

impl<A: Clone + Send + Sync> Model<A> for OpenAiModel<'_, A> {
    fn id(&self) -> &str {
        &self.id
    }

    async fn step(&mut self, req: &Request<'_>) -> Result<Reply<A>, String> {
        let request = request(req, &self.answers, &self.model)?;
        // Untyped reply: some providers add fields async-openai rejects.
        let reply: Value = self
            .client
            .chat()
            .create_byot(request)
            .await
            .map_err(|e| format!("system two: {e}"))?;
        reply_from(&reply, &self.answers, self.context_size)
    }
}
