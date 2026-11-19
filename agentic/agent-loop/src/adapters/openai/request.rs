//! Builds one chat request: protocol, rendered state, pick schema.

use std::fmt::Write;

use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequest,
    CreateChatCompletionRequestArgs, ResponseFormat, ResponseFormatJsonSchema,
};
use serde_json::json;

use super::{ANSWER, CALL, NONE};
use crate::adapters::{Answers, NONE_DESCRIPTION};
use crate::protocol::{Request, ToolOption};

pub(super) const SYSTEM: &str = "You answer one question about a codebase. Each turn, \
either pick a `call:` id to fetch one more fact, or pick an `answer:` id or \
`none` to answer. Answer only from the task and the facts. Reply with JSON: \
{\"pick\": \"<one listed id>\", \"note\": \"<one short sentence why>\"}.";
pub(in crate::adapters) fn request<A>(
    req: &Request<'_>,
    answers: &Answers<A>,
    model: &str,
) -> Result<CreateChatCompletionRequest, String> {
    let err = |e: async_openai::error::OpenAIError| format!("system two: {e}");
    let messages: Vec<ChatCompletionRequestMessage> = vec![
        ChatCompletionRequestSystemMessageArgs::default()
            .content(SYSTEM)
            .build()
            .map_err(err)?
            .into(),
        ChatCompletionRequestUserMessageArgs::default()
            .content(render(req, answers))
            .build()
            .map_err(err)?
            .into(),
    ];
    CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages)
        .temperature(0.0_f32)
        .response_format(schema(picks(req.options, answers)))
        .build()
        .map_err(err)
}

/// Every id the model may pick: calls, answers, then `none`.
pub(in crate::adapters) fn picks<A>(options: &[ToolOption], answers: &Answers<A>) -> Vec<String> {
    let calls = options.iter().map(|o| format!("{CALL}{}", o.id));
    let labels = answers
        .options
        .iter()
        .map(|o| format!("{ANSWER}{}", o.label));
    calls.chain(labels).chain([NONE.to_string()]).collect()
}

/// `strict: false`: Ollama rejects strict schemas; membership is checked in code.
fn schema(picks: Vec<String>) -> ResponseFormat {
    let schema = json!({
        "type": "object",
        "properties": {
            "pick": { "type": "string", "enum": picks },
            "note": { "type": "string" }
        },
        "required": ["pick", "note"]
    });
    ResponseFormat::JsonSchema {
        json_schema: ResponseFormatJsonSchema {
            description: Some("Call one tool or answer".to_string()),
            name: "agent_step".to_string(),
            schema: Some(schema),
            strict: Some(false),
        },
    }
}

/// Task, facts, tool options and answer options as plain text.
pub(in crate::adapters) fn render<A>(req: &Request<'_>, answers: &Answers<A>) -> String {
    let mut out = format!("Task:\n{}\n\nFacts:\n", req.task);
    for f in req.facts {
        let _ = writeln!(out, "{} [{} {}]\n{}\n", f.id.0, f.tool, f.args, f.text);
    }
    out.push_str("\nTools:\n");
    for o in req.options {
        let _ = writeln!(out, "- {CALL}{}: {}", o.id, o.description);
    }
    out.push_str("\nAnswers:\n");
    for o in &answers.options {
        let _ = writeln!(out, "- {ANSWER}{}: {}", o.label, o.description);
    }
    let _ = writeln!(out, "- {NONE}: {NONE_DESCRIPTION}");
    out
}
