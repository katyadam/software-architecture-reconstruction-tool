use std::collections::HashMap;

use log::info;
use models::{
    ParsedCallable, RestCall,
    ir::{ast::Expr, language::Language, project::ProjectIR},
};
use statix::symbolic_evaluation_with_env;

use crate::pipeline::{
    pass2::callables::mangle_callable_name,
    pass3::{
        callables::build_captured_scopes,
        env::{Env, build_constants_env, build_file_env},
        language_backend::LanguageSpecificEvaluator,
        pass_module::PerFileModuleConsts,
    },
};

use super::callables::{build_file_local_callables, build_merged_enums};
use super::language_backend::evaluation_for;

pub(super) fn evaluate_restcalls(
    project_ir: &ProjectIR,
    external_constants: &HashMap<String, String>,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
    per_file_module_consts: &PerFileModuleConsts,
) -> Vec<RestCall> {
    let merged_enums = build_merged_enums(&project_ir.files);
    let constants_env = build_constants_env(&project_ir.constants, external_constants);

    project_ir
        .files
        .iter()
        .filter(|f| !f.raw_restcalls.is_empty())
        .flat_map(|file| {
            evaluate_file_restcalls(
                file,
                project_ir,
                &merged_enums,
                &constants_env,
                per_file_attrs,
                per_file_module_consts,
            )
        })
        .collect()
}

fn evaluate_file_restcalls(
    file: &models::ir::project::TypedFileRecord,
    project_ir: &ProjectIR,
    merged_enums: &HashMap<String, Vec<String>>,
    constants_env: &Env,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
    per_file_module_consts: &PerFileModuleConsts,
) -> Vec<RestCall> {
    let callables = build_file_local_callables(file, &project_ir.callable_map);
    let evaluator: &dyn LanguageSpecificEvaluator = evaluation_for(file.language);
    let file_env = build_file_env(
        file,
        project_ir,
        constants_env,
        per_file_attrs,
        per_file_module_consts,
    );

    let captured_scopes = build_captured_scopes(
        file.callables
            .iter()
            .map(|pc| (pc.metadata.name.as_str(), &pc.ast)),
        &callables,
        evaluator,
        &file_env,
        file.language,
    );

    file.raw_restcalls
        .iter()
        .flat_map(|restcall| {
            evaluate_single_restcall(
                restcall,
                &callables,
                evaluator,
                &captured_scopes,
                &file_env,
                merged_enums,
                file.language,
            )
        })
        .collect()
}

fn evaluate_single_restcall(
    restcall: &RestCall,
    callables: &HashMap<String, ParsedCallable>,
    evaluator: &dyn LanguageSpecificEvaluator,
    captured_scopes: &HashMap<String, Env>,
    file_env: &Env,
    merged_enums: &HashMap<String, Vec<String>>,
    language: Language,
) -> Vec<RestCall> {
    if restcall.function_name.is_empty() {
        return vec![restcall.clone()];
    }

    // Prefer hash-keyed lookup to avoid mangled-name collisions between anonymous
    // functions with identical signatures (e.g. multiple `_` route handlers).
    let lookup_key =
        if !restcall.function_hash.is_empty() && callables.contains_key(&restcall.function_hash) {
            restcall.function_hash.clone()
        } else {
            mangle_callable_name(&restcall.function_name, language)
        };

    // Merge captured outer-scope env (if this callable is nested).
    let mut eval_env = file_env.clone();
    if let Some(captured) = captured_scopes.get(&restcall.function_hash) {
        for (k, v) in captured {
            eval_env.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }

    match symbolic_evaluation_with_env(callables, &lookup_key, evaluator.matcher(), &eval_env) {
        Ok(analysis) => evaluator
            .generate_uris(&restcall.target_uri, &analysis, merged_enums)
            .into_iter()
            .map(|uri| restcall.clone_from_target_uri(&uri))
            .collect(),
        Err(_) => {
            info!(
                "Symbolic Evaluation for REST call with target url: {} failed -- preserving raw REST call as-is",
                restcall.target_uri
            );
            let fallback_analysis = statix::symbolic::AnalysisResult {
                return_value: Expr::Empty,
                final_env: file_env.clone(),
            };
            evaluator
                .generate_uris(&restcall.target_uri, &fallback_analysis, merged_enums)
                .into_iter()
                .map(|uri| restcall.clone_from_target_uri(&uri))
                .collect()
        }
    }
}
