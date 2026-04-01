use crate::{CallStatement, Callable, Endpoint, Entity, RestCall};

/// Pass 3 output: fully resolved, ready for synthesis.
/// This replaces the current CodeElementsAggregate stored in S3.
pub struct EvaluatedIR {
    pub entities: Vec<Entity>,
    pub endpoints: Vec<Endpoint>, // Fully resolved URIs (with prefix chains)
    pub restcalls: Vec<RestCall>, // Fully resolved target URIs
    pub callables: Vec<Callable>,
    pub call_statements: Vec<CallStatement>,
}
