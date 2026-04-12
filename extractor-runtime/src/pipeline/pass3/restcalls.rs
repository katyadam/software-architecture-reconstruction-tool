use log::info;
use models::{RestCall, ir::project::ProjectIR};
use statix::symbolic_evaluation_with_env;

use crate::pipeline::pass3::language_backend::LanguageSpecificEvaluator;

use super::callables::{
    build_file_local_callables, build_merged_enums, build_project_global_callables,
    constants_to_env,
};
use super::language_backend::{evaluation_for, mangle_callable_name};

/// Evaluate all REST calls across the project, resolving target URIs via symbolic evaluation.
///
/// Per file:
/// 1. Build a callable map with local-priority override on the global merged map.
/// 2. For each raw rest call, run `symbolic_evaluation_with_env` seeded with constants.
/// 3. Pass the analysis result to the language-specific URI generator.
/// 4. On evaluation failure, keep the raw rest call unchanged and log the error.
pub(super) fn evaluate_restcalls(project_ir: &ProjectIR) -> Vec<RestCall> {
    let global_callables = build_project_global_callables(&project_ir.files);
    let merged_enums = build_merged_enums(&project_ir.files);
    let constants_env = constants_to_env(&project_ir.constants);

    let mut all_restcalls = Vec::new();
    for file in &project_ir.files {
        if file.raw_restcalls.is_empty() {
            continue;
        }
        let callables = build_file_local_callables(file, &global_callables);
        let evaluator: &dyn LanguageSpecificEvaluator = evaluation_for(file.language);
        for restcall in &file.raw_restcalls {
            if restcall.function_name.is_empty() {
                all_restcalls.push(restcall.clone());
                continue;
            }
            let mangled = mangle_callable_name(&restcall.function_name, file.language);
            let result = symbolic_evaluation_with_env(
                &callables,
                &mangled,
                evaluator.matcher(),
                &constants_env,
            );
            match result {
                Ok(analysis) => {
                    let uris =
                        evaluator.generate_uris(&restcall.target_uri, &analysis, &merged_enums);
                    for uri in uris {
                        all_restcalls.push(restcall.clone_from_target_uri(&uri));
                    }
                }
                Err(_) => {
                    info!(
                        "Symbolic Evaluation for REST call with target url: {} failed -- preserving raw REST call as-is",
                        restcall.target_uri
                    );
                    all_restcalls.push(restcall.clone());
                }
            }
        }
    }
    all_restcalls
}
