//! What the loop exchanges with a model and with the tools.

use std::collections::BTreeSet;

use agentic_models::{Fact, FactId};

/// One callable option. `id` encodes tool and args, e.g. `open:a.py#settings`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOption {
    pub id: String,
    pub tool: String,
    pub args: String,
    pub description: String,
}

pub struct Request<'a> {
    /// Stage prompt for this subject.
    pub task: &'a str,
    pub facts: &'a [Fact],
    /// Empty -> the model must answer.
    pub options: &'a [ToolOption],
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step<A> {
    /// Id of an offered option.
    Call(String),
    Final {
        answer: A,
        cites: Vec<FactId>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reply<A> {
    pub step: Step<A>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

pub trait Model<A> {
    /// Model name; part of the prompt hash.
    fn id(&self) -> &str;
    /// `Err` = provider failure (timeout, status, unparsable reply).
    fn step(&mut self, req: &Request<'_>) -> impl Future<Output = Result<Reply<A>, String>> + Send;
}

pub trait Tools {
    /// Options not in `taken`, best first.
    fn available(&self, taken: &BTreeSet<String>) -> Vec<ToolOption>;
    /// Tool output; the impl bounds its size.
    fn run(&self, option: &ToolOption) -> String;
}
