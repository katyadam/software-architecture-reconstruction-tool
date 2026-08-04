pub(crate) mod callables;
mod constants;
mod env;
pub(crate) mod language_backend;
pub mod pass_attr;
pub mod pass_module;
mod restcalls;

use std::collections::HashMap;

use models::{
    Endpoint,
    ir::{evaluted::EvaluatedIR, project::ProjectIR},
};

use crate::pipeline::pass3::pass_module::{MODULE_CALLABLE_NAME, PerFileModuleConsts};

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
///
/// `per_file_module_consts` is produced by `pass_module::resolve_all` and maps
/// each source file's top-level literal bindings. Imports resolving to an
/// `ImportKind::Constant` pull their value from this map into the consumer's
/// evaluation env.
pub fn evaluate(
    project_ir: ProjectIR,
    external_constants: &HashMap<String, String>,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
    per_file_module_consts: &PerFileModuleConsts,
) -> EvaluatedIR {
    let restcalls = restcalls::evaluate_restcalls(
        &project_ir,
        external_constants,
        per_file_attrs,
        per_file_module_consts,
    )
    .into_iter()
    .filter(|restcall| grpc_operation_is_declared(&restcall.target_uri, &project_ir))
    .collect();

    let endpoints: Vec<Endpoint> = project_ir
        .files
        .iter()
        .flat_map(|f| f.endpoints.clone())
        .filter(|endpoint| grpc_operation_is_declared(&endpoint.uri, &project_ir))
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
        .filter(|c| c.name != MODULE_CALLABLE_NAME)
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

/// A declared protobuf contract narrows convention-based gRPC extraction.
/// Without a matching service contract, preserve the previous behaviour.
fn grpc_operation_is_declared(uri: &str, project_ir: &ProjectIR) -> bool {
    let Some(path) = uri.strip_prefix("grpc://") else {
        return true;
    };
    let Some((service, operation)) = path.split_once('/') else {
        return true;
    };
    let contracts = project_ir
        .proto_services
        .iter()
        .filter(|contract| contract.name == service)
        .collect::<Vec<_>>();
    contracts.is_empty()
        || contracts
            .iter()
            .any(|contract| contract.operations.iter().any(|rpc| rpc == operation))
}
