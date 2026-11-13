//! One session: ask, fetch, ask again, until an answer, the budget, or an error.

use std::collections::BTreeSet;
use std::io;

use agentic_models::{Fact, FactId};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::log::{Log, prompt_hash};
use crate::protocol::{Model, Request, Step, ToolOption, Tools};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Outcome<A> {
    Answer {
        answer: A,
        cites: Vec<FactId>,
    },
    BudgetExhausted,
    /// Provider failure or invalid reply; never logged.
    ModelError(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session<A> {
    pub outcome: Outcome<A>,
    pub facts: Vec<Fact>,
    pub calls: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Read from the log, no model call.
    #[serde(skip)]
    pub replayed: bool,
}

/// Subject = point id (matcher, rematcher) or link id (verifier).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionKey {
    pub stage: String,
    pub subject: String,
}

/// Replays a logged session on a key + hash hit; otherwise talks to the model
/// and logs the session unless it ended with a model error.
pub async fn run_session<A, M, T>(
    key: &SessionKey,
    task: &str,
    model: &mut M,
    tools: &T,
    budget: usize,
    log: &Log,
) -> io::Result<Session<A>>
where
    A: Serialize + DeserializeOwned,
    M: Model<A>,
    T: Tools,
{
    let hash = prompt_hash(model.id(), budget, task);
    if let Some(mut session) = log.lookup(key, &hash) {
        session.replayed = true;
        return Ok(session);
    }
    let session = converse(task, model, tools, budget).await;
    if !matches!(session.outcome, Outcome::ModelError(_)) {
        log.append(key, &hash, &session)?;
    }
    Ok(session)
}

async fn converse<A, M: Model<A>, T: Tools>(
    task: &str,
    model: &mut M,
    tools: &T,
    budget: usize,
) -> Session<A> {
    let mut s = Session::start();
    let mut taken = BTreeSet::new();
    loop {
        let options = if s.facts.len() < budget {
            tools.available(&taken)
        } else {
            vec![]
        };
        let outcome = match ask(model, task, &mut s, &options).await {
            Err(e) => Outcome::ModelError(e),
            Ok(Step::Final { answer, cites }) => checked_answer(&s.facts, answer, cites),
            // Budget spent or nothing left to fetch.
            Ok(Step::Call(_)) if options.is_empty() => Outcome::BudgetExhausted,
            Ok(Step::Call(id)) => match options.iter().find(|o| o.id == id) {
                Some(option) => {
                    record_fact(&mut s.facts, tools, option);
                    taken.insert(id);
                    continue;
                }
                None => Outcome::ModelError(format!("call to unoffered option {id}")),
            },
        };
        s.outcome = outcome;
        return s;
    }
}

/// One model call; counts it and its tokens.
async fn ask<A, M: Model<A>>(
    model: &mut M,
    task: &str,
    s: &mut Session<A>,
    options: &[ToolOption],
) -> Result<Step<A>, String> {
    let request = Request {
        task,
        facts: &s.facts,
        options,
    };
    let reply = model.step(&request).await?;
    s.calls += 1;
    s.input_tokens += reply.input_tokens;
    s.output_tokens += reply.output_tokens;
    Ok(reply.step)
}

/// The answer, unless it cites a fact this session never recorded.
fn checked_answer<A>(facts: &[Fact], answer: A, cites: Vec<FactId>) -> Outcome<A> {
    match cites.iter().find(|c| !facts.iter().any(|f| f.id == **c)) {
        Some(c) => Outcome::ModelError(format!("answer cites unknown fact {}", c.0)),
        None => Outcome::Answer { answer, cites },
    }
}

/// Runs the tool and records its output as the next `fN`.
fn record_fact<T: Tools>(facts: &mut Vec<Fact>, tools: &T, option: &ToolOption) {
    facts.push(Fact {
        id: FactId(format!("f{}", facts.len() + 1)),
        tool: option.tool.clone(),
        args: option.args.clone(),
        text: tools.run(option),
    });
}

impl<A> Session<A> {
    fn start() -> Self {
        Self {
            outcome: Outcome::BudgetExhausted,
            facts: vec![],
            calls: 0,
            input_tokens: 0,
            output_tokens: 0,
            replayed: false,
        }
    }
}
