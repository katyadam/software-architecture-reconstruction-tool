//! LLM-assisted enhancement of unresolved REST calls.
//!
//! Decomposed into:
//! - [`dispatch`]: orchestrator + concurrent Sage dispatch
//! - [`query_builder`]: turning a [`models::RestCall`] into a [`sage::resolver::query::SageQuery`]
//! - [`variables`]: file -> microservice resolution helper
//! - [`tokens`]: shared identifier splitters (camel/snake)
//! - [`matcher`]: deterministic identifier -> service matcher (Phase 2a)
//! - [`oracle`]: auto-derived ground-truth oracle (S0.2)
//! - [`scorer`]: precision/recall scorer (S0.2)
//! - [`residual_edge_filter`]: residual cross-service vs non-edge classifier (Phase 1.5)

mod baseline;
mod dispatch;
mod matcher;
mod oracle;
mod query_builder;
mod residual_edge_filter;
mod scorer;
mod signals;
mod tokens;
mod variables;

pub use dispatch::evaluate_restcalls_with_llm;
