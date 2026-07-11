use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs, ResponseFormat,
        ResponseFormatJsonSchema,
    },
};

pub use crate::resolver::query::{CandidateService, QueryKind, SageQuery};
pub use crate::resolver::response::{SageError, SageResponse};

use crate::resolver::{
    prompt::{build_context_message, build_system_message, context_grounding_string},
    response::{LlmJson, validate},
};

/// HTTP client that sends closed-set classification queries to an Ollama-backed LLM.
#[derive(Debug)]
pub struct SageClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl SageClient {
    /// Create a client pointing at `base_url` (e.g. `"http://localhost:11434/v1"`).
    pub fn new(base_url: &str, model: &str) -> Self {
        let config = OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key("ollama");
        Self {
            client: Client::with_config(config),
            model: model.to_string(),
        }
    }

    /// Send a classification query and return a validated response.
    pub async fn query(&self, query: SageQuery) -> Result<SageResponse, SageError> {
        let QueryKind::ClassifyTargetService { candidates } = &query.kind;

        let system_msg = build_system_message();
        let context_msg = build_context_message(&query.context, candidates);

        let messages: Vec<ChatCompletionRequestMessage> = vec![
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system_msg)
                .build()
                .map_err(SageError::Network)?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(context_msg)
                .build()
                .map_err(SageError::Network)?
                .into(),
        ];

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages)
            .response_format(classify_response_format(candidates))
            .build()
            .map_err(SageError::Network)?;

        let response = self.client.chat().create(request).await?;

        let text = response
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default();

        let grounding = context_grounding_string(&query.context);

        // Compute the outcome into a local Result so it can be traced even when
        // parsing or validation fails -- rejection cases are what we measure.
        let outcome = serde_json::from_str::<LlmJson>(text.trim())
            .map_err(SageError::from)
            .and_then(|parsed| validate(parsed, candidates, &grounding));

        trace_query(&query, &text, &outcome);

        outcome
    }
}

/// Build the Ollama structured-output schema (plan §6.3): `service` constrained
/// to the candidate names plus null, `evidence` an array of strings, `reasoning`
/// a string. `strict` is `false` -- Ollama rejects `strict: true` with a nullable
/// enum, and membership is enforced in `validate` regardless.
fn classify_response_format(candidates: &[CandidateService]) -> ResponseFormat {
    let mut service_enum: Vec<serde_json::Value> = candidates
        .iter()
        .map(|c| serde_json::Value::String(c.name.clone()))
        .collect();
    service_enum.push(serde_json::Value::Null);

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "service": { "type": ["string", "null"], "enum": service_enum },
            "evidence": { "type": "array", "items": { "type": "string" } },
            "reasoning": { "type": "string" }
        },
        "required": ["service", "evidence"]
    });

    ResponseFormat::JsonSchema {
        json_schema: ResponseFormatJsonSchema {
            description: Some("Closed-set target-service classification".to_string()),
            name: "target_service_classification".to_string(),
            schema: Some(schema),
            strict: Some(false),
        },
    }
}

/// A short human-readable label for a query kind, e.g. `"ClassifyTargetService:20"`.
fn describe_kind(kind: &QueryKind) -> String {
    match kind {
        QueryKind::ClassifyTargetService { candidates } => {
            format!("ClassifyTargetService:{}", candidates.len())
        }
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

    let candidate_count = match &query.kind {
        QueryKind::ClassifyTargetService { candidates } => candidates.len(),
    };

    let outcome_json = match outcome {
        Ok(resp) => serde_json::json!({
            "status": "resolved",
            "service": resp.service,
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
        "expression": query.context.expression,
        "candidates": candidate_count,
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

    let mut file = match std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
    {
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
