mod callables;
mod language_backend;
mod restcalls;

use models::{
    Endpoint,
    ir::{evaluted::EvaluatedIR, project::ProjectIR},
};

/// Pass 3: Produce `EvaluatedIR` from `ProjectIR`.
///
/// Implements:
///   - Cross-file symbolic evaluation with constant injection
///   - URI resolution for REST calls (Java and Python)
pub fn evaluate(project_ir: ProjectIR) -> EvaluatedIR {
    let restcalls = restcalls::evaluate_restcalls(&project_ir);

    let endpoints: Vec<Endpoint> = project_ir
        .files
        .iter()
        .flat_map(|f| f.endpoints.clone())
        .collect();

    let entities = project_ir
        .files
        .iter()
        .flat_map(|f| f.entities.clone())
        .collect();

    let callables = project_ir
        .files
        .iter()
        .flat_map(|f| f.callables.iter().map(|pc| pc.metadata.clone()))
        .collect();

    let call_statements = project_ir
        .files
        .iter()
        .flat_map(|f| f.call_statements.clone())
        .collect();

    EvaluatedIR {
        entities,
        endpoints,
        restcalls,
        callables,
        call_statements,
    }
}
