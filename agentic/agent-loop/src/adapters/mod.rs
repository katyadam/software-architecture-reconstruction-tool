//! `Model` implementations: Jev (System One) and any OpenAI-compatible endpoint.
//! Both pick from a closed set; the stage supplies the answer options.

mod jev;
mod openai;

pub use jev::{JevLimits, JevModel, JevWording};
pub use openai::{OpenAiModel, system_two_client};

/// One answer the model may pick, by label.
#[derive(Debug, Clone, PartialEq)]
pub struct AnswerOption<A> {
    pub label: String,
    pub description: String,
    pub value: A,
}

/// A session's answer set, plus the value used when nothing fits.
#[derive(Debug, Clone, PartialEq)]
pub struct Answers<A> {
    pub options: Vec<AnswerOption<A>>,
    pub none: A,
}

/// Description of the `none` pick, shared by both adapters.
const NONE_DESCRIPTION: &str = "None of these fits, or the facts do not show it.";

/// `model#<12 hex>`: a model id that changes when `settings` change.
fn fingerprint(model: &str, settings: &str) -> String {
    format!("{model}#{}", &crate::log::sha_hex(settings)[..12])
}

impl<A: Clone> Answers<A> {
    fn value_of(&self, label: &str) -> Option<A> {
        self.options
            .iter()
            .find(|o| o.label == label)
            .map(|o| o.value.clone())
    }
}

#[cfg(test)]
mod tests;
