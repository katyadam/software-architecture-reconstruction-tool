use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
};

pub use crate::resolver::query::{QueryKind, SageQuery};
pub use crate::resolver::response::{SageError, SageResponse};

use crate::resolver::{
    prompt::{
        build_facts_message, build_question_message, build_system_message, build_variables_message,
    },
    response::{LlmJson, validate},
};

/// HTTP client that sends resolution queries to an Ollama-backed LLM.
#[derive(Debug)]
pub struct SageClient {
    client: Client<OpenAIConfig>,
    model: String,
    confidence_threshold: f32,
    variables_budget: usize,
}

impl SageClient {
    /// Create a client pointing at `base_url` (e.g. `"http://localhost:11434/v1"`).
    ///
    /// `variables_budget` caps the number of variable entries included in each
    /// query prompt. Callers should rank and prune the full variables map down
    /// to at most this many entries before sending.
    pub fn new(
        base_url: &str,
        model: &str,
        confidence_threshold: f32,
        variables_budget: usize,
    ) -> Self {
        let config = OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key("ollama");
        Self {
            client: Client::with_config(config),
            model: model.to_string(),
            confidence_threshold,
            variables_budget,
        }
    }

    pub fn variables_budget(&self) -> usize {
        self.variables_budget
    }

    /// Send a resolution query and return a validated response.
    pub async fn query(&self, query: SageQuery) -> Result<SageResponse, SageError> {
        let system_msg = build_system_message();
        let facts_msg = build_facts_message(&query.bundle);
        let variables_msg = build_variables_message(query.variables_map.as_slice());
        let question_msg = build_question_message(&query.kind);

        let messages: Vec<ChatCompletionRequestMessage> = vec![
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system_msg)
                .build()
                .map_err(SageError::Network)?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(facts_msg)
                .build()
                .map_err(SageError::Network)?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(variables_msg)
                .build()
                .map_err(SageError::Network)?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(question_msg)
                .build()
                .map_err(SageError::Network)?
                .into(),
        ];

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages)
            .build()
            .map_err(SageError::Network)?;

        let response = self.client.chat().create(request).await?;

        let text = response
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default();

        // Compute the outcome into a local Result so it can be traced even when
        // parsing or validation fails -- rejection cases are what we measure.
        let outcome = serde_json::from_str::<LlmJson>(text.trim())
            .map_err(SageError::from)
            .and_then(|parsed| validate(parsed, self.confidence_threshold));

        trace_query(&query, &text, &outcome);

        outcome
    }
}

/// A short human-readable label for a query kind, e.g. `"ResolveLookup:user-service"`.
fn describe_kind(kind: &QueryKind) -> String {
    match kind {
        QueryKind::ResolveEnvVar { var_name } => format!("ResolveEnvVar:{var_name}"),
        QueryKind::ResolveBuilder { chain } => format!("ResolveBuilder:{chain}"),
        QueryKind::ResolveLookup { lookup_key } => format!("ResolveLookup:{lookup_key}"),
        QueryKind::ResolveFrameworkRoute { route_pattern } => {
            format!("ResolveFrameworkRoute:{route_pattern}")
        }
        QueryKind::ResolveReflective { target } => format!("ResolveReflective:{target}"),
        QueryKind::ClassifyHttpCall { call_expr } => format!("ClassifyHttpCall:{call_expr}"),
    }
}

/// Append one JSONL record per query to the file named by `SAGE_TRACE`.
///
/// No-op when `SAGE_TRACE` is unset. Never panics or propagates IO errors --
/// tracing must not break a real run.
fn trace_query(query: &SageQuery, raw_response: &str, outcome: &Result<SageResponse, SageError>) {
    let path = match std::env::var_os("SAGE_TRACE") {
        Some(p) => p,
        None => return,
    };

    let snippet = query
        .bundle
        .sites
        .first()
        .map(|s| s.code.as_str())
        .unwrap_or("");

    let outcome_json = match outcome {
        Ok(resp) => serde_json::json!({
            "status": "resolved",
            "resolved": resp.resolved,
            "confidence": resp.confidence,
            "evidence": resp.evidence,
            "reasoning": resp.reasoning,
        }),
        Err(err) => serde_json::json!({
            "status": "rejected",
            "error": err.to_string(),
        }),
    };

    let record = serde_json::json!({
        "kind": describe_kind(&query.kind),
        "snippet": snippet,
        "variables_offered": query.variables_map.len(),
        "raw_response": raw_response,
        "outcome": outcome_json,
    });

    let line = match serde_json::to_string(&record) {
        Ok(l) => l,
        Err(e) => {
            log::warn!("SAGE_TRACE: failed to serialize trace record: {e}");
            return;
        }
    };

    let mut file = match std::fs::OpenOptions::new().append(true).create(true).open(&path) {
        Ok(f) => f,
        Err(e) => {
            log::warn!("SAGE_TRACE: failed to open trace file {path:?}: {e}");
            return;
        }
    };

    use std::io::Write;
    if let Err(e) = writeln!(file, "{line}") {
        log::warn!("SAGE_TRACE: failed to write trace record: {e}");
    }
}
