pub(crate) mod callables;
mod constants;
mod env;
pub(crate) mod language_backend;
mod llm_enhance;
mod message_edges;
pub mod pass_attr;
pub mod pass_module;
mod restcalls;
mod route_prefixes;

use std::collections::HashMap;

use clients::typesafe::TypeSafeClient;
use models::{
    ConfigurationData, RestCall,
    ir::{evaluted::EvaluatedIR, project::ProjectIR},
};

use crate::pipeline::pass3::{
    llm_enhance::evaluate_restcalls_with_llm,
    pass_module::{MODULE_CALLABLE_NAME, PerFileModuleConsts},
};

pub use llm_enhance::{DdfaMode, Resolver};
pub use restcalls::OperandBindings;

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
    project_ir: &ProjectIR,
    external_constants: &HashMap<String, String>,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
    per_file_module_consts: &PerFileModuleConsts,
) -> EvaluatedIR {
    evaluate_with_bindings(
        project_ir,
        external_constants,
        per_file_attrs,
        per_file_module_consts,
    )
    .0
}

/// `evaluate`, plus each REST call's operand bindings: `bindings[i]` belongs
/// to `restcalls[i]`, and is empty unless that call stayed a residual.
pub fn evaluate_with_bindings(
    project_ir: &ProjectIR,
    external_constants: &HashMap<String, String>,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
    per_file_module_consts: &PerFileModuleConsts,
) -> (EvaluatedIR, Vec<OperandBindings>) {
    let (restcalls, bindings): (Vec<_>, Vec<_>) = restcalls::evaluate_restcalls(
        project_ir,
        external_constants,
        per_file_attrs,
        per_file_module_consts,
    )
    .into_iter()
    .unzip();
    let ir = assemble(
        project_ir,
        restcalls,
        external_constants,
        per_file_attrs,
        per_file_module_consts,
    );
    (ir, bindings)
}

/// Pass 3 with residual resolution by `resolver`.
pub async fn evaluate_with_llm(
    project_ir: ProjectIR,
    external_constants: &HashMap<String, String>,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
    per_file_module_consts: &PerFileModuleConsts,
    config: &ConfigurationData,
    resolver: Resolver<'_>,
    typesafe_client: Option<&TypeSafeClient>,
) -> EvaluatedIR {
    let (mut restcalls, bindings): (Vec<_>, Vec<_>) = restcalls::evaluate_restcalls(
        &project_ir,
        external_constants,
        per_file_attrs,
        per_file_module_consts,
    )
    .into_iter()
    .unzip();

    evaluate_restcalls_with_llm(
        &mut restcalls,
        &bindings,
        config,
        external_constants,
        resolver,
        typesafe_client,
        &project_ir,
    )
    .await;
    assemble(
        &project_ir,
        restcalls,
        external_constants,
        per_file_attrs,
        per_file_module_consts,
    )
}

/// Everything but the REST calls, evaluated from `project_ir`.
fn assemble(
    project_ir: &ProjectIR,
    restcalls: Vec<RestCall>,
    external_constants: &HashMap<String, String>,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
    per_file_module_consts: &PerFileModuleConsts,
) -> EvaluatedIR {
    let message_edges = message_edges::evaluate_message_edges(
        project_ir,
        external_constants,
        per_file_attrs,
        per_file_module_consts,
    );
    let endpoints = route_prefixes::evaluate_endpoints(
        project_ir,
        external_constants,
        per_file_attrs,
        per_file_module_consts,
    );
    let files = &project_ir.files;
    EvaluatedIR {
        entities: files.iter().flat_map(|f| f.entities.clone()).collect(),
        endpoints,
        restcalls,
        message_edges,
        callables: files
            .iter()
            .flat_map(|f| f.callables.iter().map(|pc| pc.metadata.clone()))
            .filter(|c| c.name != MODULE_CALLABLE_NAME)
            .collect(),
        call_statements: files
            .iter()
            .flat_map(|f| f.call_statements.iter().map(|c| c.metadata.clone()))
            .collect(),
        resolved_call_edges: vec![],
    }
}
