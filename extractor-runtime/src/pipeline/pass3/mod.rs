mod callables;
mod language_backend;
mod restcalls;

use std::collections::HashMap;

use models::{
    Endpoint,
    ir::{evaluted::EvaluatedIR, project::ProjectIR},
};

/// Pass 3: Produce `EvaluatedIR` from `ProjectIR`.
///
/// `external_constants` may contain dotted-path keys (e.g. `"settings.as_url"`)
/// that are injected into the symbolic evaluation environment alongside
/// the in-source constants collected in Pass 2.
///
/// `per_file_attrs` is produced by `pass_attr::resolve_all` and provides
/// per-file attribute defaults derived from class field declarations.  These
/// fill in only where neither project constants nor `external_constants` set a
/// value (lowest priority among explicit sources).
pub fn evaluate(
    project_ir: ProjectIR,
    external_constants: &HashMap<String, String>,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
) -> EvaluatedIR {
    let restcalls = restcalls::evaluate_restcalls(&project_ir, external_constants, per_file_attrs);

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
