//! LLM-assisted enhancement of unresolved REST calls.
//!
//! Decomposed into:
//! - [`dispatch`]: orchestrator + concurrent Sage dispatch
//! - [`query_builder`]: turning a [`models::RestCall`] into a [`sage::resolver::query::SageQuery`]
//! - [`variables`]: the canonical variable map builder consumed by Pass 3
//! - [`ranking`]: relevance ranking + secret redaction for variable maps

mod dispatch;
mod query_builder;
mod ranking;
mod variables;

pub use dispatch::evaluate_restcalls_with_llm;
pub use variables::build_variable_map;
