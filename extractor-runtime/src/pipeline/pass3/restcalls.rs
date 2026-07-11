use std::collections::HashMap;

use log::info;
use models::{
    ParsedCallable, RestCall,
    ir::{language::Language, project::ProjectIR},
};
use statix::{symbolic::AnalysisResult, symbolic_evaluation_with_env};

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
    let Some(evaluator) = evaluation_for(file.language) else {
        return vec![];
    };
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
            // Symbolic evaluation needs the enclosing callable's env; when that
            // lookup fails (e.g. a Java test method absent from the callable map)
            // we can still resolve any part of the template that is a pure string
            // literal -- those need no env. Running `generate_uris` against an
            // empty analysis strips inline-literal quotes and concatenates literal
            // parts (so `"http://x/" + "y"` -> `http://x/y`), while genuinely
            // env-dependent variables fall through unchanged and stay residual.
            // This prevents a fully-known literal URL from being misfiled as an
            // unresolved residual just because its quotes survived.
            info!(
                "Symbolic Evaluation for REST call with target url: {} failed -- resolving literals only",
                restcall.target_uri
            );
            evaluator
                .generate_uris(&restcall.target_uri, &AnalysisResult::default(), merged_enums)
                .into_iter()
                .map(|uri| restcall.clone_from_target_uri(&uri))
                .collect()
        }
    }
}

#[derive(PartialEq, Eq)]
pub(super) enum EvalState {
    ResolvedURL,     // eval produced a concrete http... URL; skip resolution
    NeedsResolution, // non-empty residual eval did not turn into a URL; resolve it
    Junk,            // genuinely empty; nothing to resolve
}

/// Structural gate over a symbolically-evaluated `RestCall`.
///
/// A `RestCall` already IS an HTTP call (it carries `http_method`); any
/// non-empty residual that eval did not turn into an http URL is a real
/// residual the Phase 2 matcher should try, regardless of how it is named. The
/// only thing not worth forwarding is an empty `target_uri` -> there is nothing
/// to resolve.
pub(super) fn is_restcall_evaluated_enough(restcall: &RestCall) -> EvalState {
    if restcall.target_uri.is_empty() {
        EvalState::Junk
    } else if restcall.target_uri.starts_with("http") {
        EvalState::ResolvedURL
    } else {
        EvalState::NeedsResolution
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::source_code::SourceSpan;

    fn restcall(target_uri: &str) -> RestCall {
        RestCall {
            function_name: "do_call".to_string(),
            function_hash: "h1".to_string(),
            call_arguments: vec![],
            http_method: Default::default(),
            target_uri: target_uri.to_string(),
            file_path: "/proj/caller/client.py".to_string(),
            source_span: SourceSpan::new(0, 0),
        }
    }

    #[test]
    fn empty_target_uri_is_junk() {
        assert!(is_restcall_evaluated_enough(&restcall("")) == EvalState::Junk);
    }

    #[test]
    fn http_url_is_resolved() {
        assert!(
            is_restcall_evaluated_enough(&restcall("http://medical-data-service:8000/x"))
                == EvalState::ResolvedURL
        );
    }

    #[test]
    fn https_url_is_resolved() {
        assert!(
            is_restcall_evaluated_enough(&restcall("https://medical-data-service:8000/x"))
                == EvalState::ResolvedURL
        );
    }

    #[test]
    fn url_named_residual_needs_resolution() {
        // Was NeedsLLM under the old lexical gate; still forwarded.
        assert!(
            is_restcall_evaluated_enough(&restcall("self._mds_url + url"))
                == EvalState::NeedsResolution
        );
    }

    #[test]
    fn bare_path_param_needs_resolution() {
        // CRITICAL: under the OLD gate this was Junk -- the first `/`-segment
        // "case_id" carries no url/uri token -- and it is exactly a recovered
        // case. The structural gate now forwards it.
        assert!(is_restcall_evaluated_enough(&restcall("case_id")) == EvalState::NeedsResolution);
    }

    #[test]
    fn non_url_token_residual_needs_resolution() {
        // Another previously-Junk case: no url/uri token anywhere.
        assert!(
            is_restcall_evaluated_enough(&restcall("self._client_base + path"))
                == EvalState::NeedsResolution
        );
    }
}
