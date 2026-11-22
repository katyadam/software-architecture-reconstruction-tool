//! The two Jev stages, their question texts, and how models are built.

use agent_loop::Model;
use agent_loop::adapters::{Answers, JevWording};
use clients::typesafe::question::NoulCriteria;

/// What a session picked; `None` when nothing fits.
pub type Pick = Option<Target>;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Target {
    /// A service name, or a receiver point id in the endpoint stage.
    Point(String),
    /// A call to a system outside the configured services.
    External,
    /// No call to a service's API: a map or cache, a data-store client or a library.
    NotAService,
}

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
                "Which service does this call reach? A call on an in-memory map or cache, \
                 a database or other data-store client, or a library/SDK object is not a \
                 service call: answer not-a-service. \
                 Pick the caller's own service only for an HTTP call to its own endpoints.",
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
