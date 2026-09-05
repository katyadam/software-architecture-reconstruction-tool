use crate::{
    CallStatement, Callable, CodeElementsAggregate, Endpoint, Entity, MessageEdge, RestCall,
};

/// Pass 3 output: fully resolved, ready for synthesis.
/// This replaces the current CodeElementsAggregate stored in S3.
#[derive(Debug)]
pub struct EvaluatedIR {
    pub entities: Vec<Entity>,
    pub endpoints: Vec<Endpoint>, // Fully resolved URIs (with prefix chains)
    pub restcalls: Vec<RestCall>, // Fully resolved target URIs
    pub message_edges: Vec<MessageEdge>,
    pub callables: Vec<Callable>,
    pub call_statements: Vec<CallStatement>,
}

impl From<EvaluatedIR> for CodeElementsAggregate {
    fn from(ir: EvaluatedIR) -> Self {
        // TODO: remove imports, they are not tracked in EvaluatedIR — synthesizer does not use them
        CodeElementsAggregate::new(
            vec![],
            ir.entities,
            ir.endpoints,
            ir.restcalls,
            ir.message_edges,
            ir.callables,
            ir.call_statements,
        )
    }
}
