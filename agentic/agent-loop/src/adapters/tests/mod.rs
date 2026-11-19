use super::{AnswerOption, Answers};
use crate::protocol::ToolOption;

mod jev;
mod openai;

/// `orders` and `payments`; `none` -> empty string.
fn answers() -> Answers<String> {
    let option = |label: &str| AnswerOption {
        label: label.into(),
        description: format!("service {label}"),
        value: label.to_uppercase(),
    };
    Answers {
        options: vec![option("orders"), option("payments")],
        none: String::new(),
    }
}

fn options(ids: &[&str]) -> Vec<ToolOption> {
    ids.iter()
        .map(|id| ToolOption {
            id: id.to_string(),
            tool: "open_file".into(),
            args: id.to_string(),
            description: format!("open {id}"),
        })
        .collect()
}
