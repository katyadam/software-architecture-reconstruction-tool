//! The two Jev stages, their question texts, and how models are built.

use agent_loop::Model;
use agent_loop::adapters::{Answers, JevWording};
use clients::typesafe::question::NoulCriteria;

/// A service name or a receiver point id; `None` when nothing fits.
pub type Pick = Option<String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Service,
    Endpoint,
}

impl Stage {
    /// Log stage name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Service => "match-service",
            Self::Endpoint => "match-endpoint",
        }
    }

    pub fn wording(self) -> JevWording {
        let (answer, enough) = match self {
            Self::Service => (
                "Which service does this call reach?",
                "Do the facts show which service this call reaches?",
            ),
            Self::Endpoint => (
                "Which endpoint of the picked service does this call reach?",
                "Do the facts show which endpoint this call reaches?",
            ),
        };
        JevWording {
            answer: answer.into(),
            enough: enough.into(),
            enough_criteria: NoulCriteria {
                yes: "the facts name the target".into(),
                no: "more facts are needed".into(),
            },
            next: "Which fact would help most?".into(),
        }
    }
}

/// Builds one model per session, given that session's answer set.
pub trait JevFactory {
    type M<'a>: Model<Pick>
    where
        Self: 'a;
    fn model(&self, stage: Stage, answers: Answers<Pick>) -> Self::M<'_>;
}
