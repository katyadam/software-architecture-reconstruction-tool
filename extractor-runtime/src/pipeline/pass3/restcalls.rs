use std::collections::{BTreeMap, HashMap};

use log::info;
use models::{
    ParsedCallable, RestCall,
    ir::{ast::Expr, language::Language, project::ProjectIR},
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

/// Residual operand -> rendered `final_env` binding, e.g. `self._mds_url` -> `settings.mds_url`.
pub(crate) type OperandBindings = BTreeMap<String, String>;

/// Bindings are empty unless the output is a residual.
pub(super) fn evaluate_restcalls(
    project_ir: &ProjectIR,
    external_constants: &HashMap<String, String>,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
    per_file_module_consts: &PerFileModuleConsts,
) -> Vec<(RestCall, OperandBindings)> {
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
) -> Vec<(RestCall, OperandBindings)> {
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
) -> Vec<(RestCall, OperandBindings)> {
    if restcall.function_name.is_empty() {
        return vec![(restcall.clone(), OperandBindings::new())];
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
        Ok(analysis) => with_bindings(
            restcall,
            evaluator.generate_uris(&restcall.target_uri, &analysis, merged_enums),
            &analysis.final_env,
        ),
        Err(_) => {
            // Symbolic evaluation needs the enclosing callable's env; when that
            // lookup fails (e.g. a Java test method absent from the callable map)
            // we can still resolve any part of the template that is a pure string
            // literal -- those need no env. Running `generate_uris` against the
            // file env strips inline-literal quotes and concatenates literal
            // parts (so `"http://x/" + "y"` -> `http://x/y`), resolves file-level
            // names, and leaves the remaining variables residual.
            // This prevents a fully-known literal URL from being misfiled as an
            // unresolved residual just because its quotes survived.
            info!(
                "Symbolic Evaluation for REST call with target url: {} failed -- resolving literals only",
                restcall.target_uri
            );
            let fallback_analysis = AnalysisResult {
                return_value: Expr::Empty,
                final_env: file_env.clone(),
            };
            with_bindings(
                restcall,
                evaluator.generate_uris(&restcall.target_uri, &fallback_analysis, merged_enums),
                &fallback_analysis.final_env,
            )
        }
    }
}

fn with_bindings(
    restcall: &RestCall,
    uris: Vec<String>,
    env: &Env,
) -> Vec<(RestCall, OperandBindings)> {
    uris.into_iter()
        .map(|uri| {
            let rc = restcall.clone_from_target_uri(&uri);
            let bindings = if is_restcall_evaluated_enough(&rc) == EvalState::NeedsResolution {
                operand_bindings(&restcall.target_uri, env)
            } else {
                OperandBindings::new()
            };
            (rc, bindings)
        })
        .collect()
}

/// Operands are read off the template, not the residual: residual text is
/// language-specific (Python drops `+`, f-strings keep `{var}`).
/// Literal-bound operands were resolved, so they are skipped.
// ponytail: `final_env` is end-of-function state, not call-site state.
fn operand_bindings(template: &str, env: &Env) -> OperandBindings {
    template
        .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
        .map(|token| token.trim_end_matches(".value"))
        .filter_map(|token| match env.get(token)? {
            (_, Expr::Literal(_)) => None,
            (dtype, expr) => Some((token.to_string(), render_binding(token, dtype, expr))),
        })
        .collect()
}

/// `url` bound to itself -> `unresolved: str`; else the expression, plus type.
fn render_binding(name: &str, dtype: &Option<String>, expr: &Expr) -> String {
    let value = match expr {
        Expr::Var(v) if v == name => "unresolved".to_string(),
        e => render_expr(e),
    };
    match dtype {
        Some(t) => format!("{value}: {t}"),
        None => value,
    }
}

fn render_expr(expr: &Expr) -> String {
    match expr {
        Expr::Literal(s) => format!("{s:?}"),
        Expr::Var(v) => v.clone(),
        Expr::Concat(a, b) => format!("{} + {}", render_expr(a), render_expr(b)),
        Expr::StructLiteral { type_name, .. } => {
            format!("{}{{..}}", type_name.as_deref().unwrap_or_default())
        }
        Expr::Call {
            name,
            receiver,
            args,
        } => {
            let args: Vec<String> = args.iter().map(render_expr).collect();
            match receiver {
                Some(r) => format!("{}.{name}({})", render_expr(r), args.join(", ")),
                None => format!("{name}({})", args.join(", ")),
            }
        }
        Expr::Empty => "?".to_string(),
        Expr::Joined { vals } => vals.iter().map(render_expr).collect::<Vec<_>>().join(" | "),
        Expr::Attr { object, field } => format!("{}.{field}", render_expr(object)),
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

    #[test]
    fn operand_bindings_render_non_literal_operands() {
        let env: Env = HashMap::from([
            (
                "self._mds_url".to_string(),
                (
                    None,
                    Expr::Attr {
                        object: Box::new(Expr::Var("settings".to_string())),
                        field: "mds_url".to_string(),
                    },
                ),
            ),
            (
                "case_id".to_string(),
                (Some("str".to_string()), Expr::Var("case_id".to_string())),
            ),
            (
                "PREFIX".to_string(),
                (None, Expr::Literal("/v1".to_string())),
            ),
        ]);

        let bindings = operand_bindings("self._mds_url + PREFIX + \"/cases/\" + case_id", &env);

        assert_eq!(
            bindings,
            OperandBindings::from([
                ("case_id".to_string(), "unresolved: str".to_string()),
                ("self._mds_url".to_string(), "settings.mds_url".to_string()),
            ])
        );
    }
}
