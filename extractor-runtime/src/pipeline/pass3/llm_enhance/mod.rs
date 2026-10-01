//! Resolution of residual (unresolved) REST call targets.

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
