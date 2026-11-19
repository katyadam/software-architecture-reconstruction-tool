//! Maps one chat reply to a step, checking picks and context overflow.

use serde::Deserialize;
use serde_json::Value;

use super::{ANSWER, CALL, NONE};
use crate::adapters::Answers;
use crate::protocol::{Answer, Reply, Step};

#[derive(Deserialize)]
struct Pick {
    pick: String,
    note: Option<String>,
}

pub(in crate::adapters) fn reply_from<A: Clone>(
    reply: &Value,
    answers: &Answers<A>,
    context_size: Option<u32>,
) -> Result<Reply<A>, String> {
    let (input_tokens, output_tokens) = usage(reply, context_size)?;
    let content = reply["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("system two: reply has no content")?;
    // ponytail: no ```json fence stripping; check real system-two replies in the verifier change.
    let pick: Pick = serde_json::from_str(content.trim())
        .map_err(|e| format!("system two: unparsable reply: {e}"))?;
    Ok(Reply {
        step: step_of(pick, answers)?,
        input_tokens,
        output_tokens,
    })
}

fn step_of<A: Clone>(pick: Pick, answers: &Answers<A>) -> Result<Step<A>, String> {
    // Any call goes to the loop: it rejects unoffered ids and ends a
    // spent budget as exhausted.
    if let Some(id) = pick.pick.strip_prefix(CALL) {
        return Ok(Step::Call(id.to_string()));
    }
    let value = if pick.pick == NONE {
        Some(answers.none.clone())
    } else {
        pick.pick
            .strip_prefix(ANSWER)
            .and_then(|label| answers.value_of(label))
    };
    match value {
        Some(value) => Ok(Step::Final(Answer {
            value,
            confidence: None,
            note: pick.note,
        })),
        None => Err(format!("system two: pick {} was not offered", pick.pick)),
    }
}

/// Prompt and completion tokens. With a context size, a prompt that fills it
/// was likely truncated (Ollama does so silently), so the step fails.
pub(in crate::adapters) fn usage(
    reply: &Value,
    context_size: Option<u32>,
) -> Result<(u64, u64), String> {
    let prompt = reply["usage"]["prompt_tokens"].as_u64();
    let completion = reply["usage"]["completion_tokens"].as_u64().unwrap_or(0);
    match (context_size, prompt) {
        (Some(size), None) => Err(format!(
            "system two: reply has no prompt token count, cannot check context size {size}"
        )),
        (Some(size), Some(p)) if p >= u64::from(size) => Err(format!(
            "system two: prompt filled the context ({p} >= {size} tokens), likely truncated"
        )),
        (_, p) => Ok((p.unwrap_or(0), completion)),
    }
}
