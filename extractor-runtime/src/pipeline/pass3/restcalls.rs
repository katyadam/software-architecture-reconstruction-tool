use std::collections::HashMap;

use log::info;
use models::{
    ParsedCallable, RestCall,
    ir::{
        ast::{CallableAst, Expr},
        language::Language,
        project::ProjectIR,
    },
};
use statix::symbolic_evaluation_with_env;

use crate::pipeline::pass3::language_backend::LanguageSpecificEvaluator;

use super::callables::{
    build_file_local_callables, build_merged_enums, build_project_global_callables,
    constants_to_env,
};
use super::language_backend::{evaluation_for, mangle_callable_name};

type Env = HashMap<String, (Option<String>, Expr)>;

pub(super) fn evaluate_restcalls(
    project_ir: &ProjectIR,
    external_constants: &HashMap<String, String>,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
) -> Vec<RestCall> {
    let global_callables = build_project_global_callables(&project_ir.files);
    let merged_enums = build_merged_enums(&project_ir.files);
    let constants_env = build_constants_env(&project_ir.constants, external_constants);

    project_ir
        .files
        .iter()
        .filter(|f| !f.raw_restcalls.is_empty())
        .flat_map(|file| {
            evaluate_file_restcalls(
                file,
                &global_callables,
                &merged_enums,
                &constants_env,
                per_file_attrs,
            )
        })
        .collect()
}

/// Merge project-scanned constants with CLI-supplied external constants into one env.
/// External constants are lower priority — they don't overwrite project-scanned values.
/// Dotted-path keys like `"settings.as_url"` are preserved as-is.
fn build_constants_env(
    project_constants: &HashMap<String, models::ir::project::ConstantValue>,
    external_constants: &HashMap<String, String>,
) -> Env {
    let mut env = constants_to_env(project_constants);
    for (name, value) in external_constants {
        let trimmed = value
            .trim_matches(|c: char| c == '"' || c == '\'')
            .to_string();
        env.entry(name.clone())
            .or_insert_with(|| (Some("String".to_string()), Expr::Literal(trimmed)));
    }
    env
}

fn evaluate_file_restcalls(
    file: &models::ir::project::TypedFileRecord,
    global_callables: &HashMap<String, ParsedCallable>,
    merged_enums: &HashMap<String, Vec<String>>,
    constants_env: &Env,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
) -> Vec<RestCall> {
    let callables = build_file_local_callables(file, global_callables);
    let evaluator: &dyn LanguageSpecificEvaluator = evaluation_for(file.language);

    // Build a file-specific env that layers per-file attribute defaults below
    // the shared constants env (project constants + CLI external_constants).
    // `or_insert_with` ensures the shared env always wins on key collisions.
    let mut file_env: Env = constants_env.clone();
    if let Some(attrs) = per_file_attrs.get(&file.file_path) {
        for (k, v) in attrs {
            file_env
                .entry(k.clone())
                .or_insert_with(|| (Some("String".to_string()), Expr::Literal(v.clone())));
        }
    }

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

    let mangled = mangle_callable_name(&restcall.function_name, language);

    // Merge captured outer-scope env (if this callable is nested).
    // Key by function_hash (unique per function body) so sibling inner
    // functions with identical signatures don't collide.
    let mut eval_env = file_env.clone();
    if let Some(captured) = captured_scopes.get(&restcall.function_hash) {
        for (k, v) in captured {
            eval_env.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }

    match symbolic_evaluation_with_env(callables, &mangled, evaluator.matcher(), &eval_env) {
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
            vec![restcall.clone()]
        }
    }
}

/// For each outer callable that has nested refs, symbolically evaluate it once
/// (with the constants env) to populate a map from inner callable key -> captured Env.
fn build_captured_scopes<'a>(
    callables: impl Iterator<Item = (&'a str, &'a CallableAst)>,
    callables_map: &HashMap<String, ParsedCallable>,
    evaluator: &dyn LanguageSpecificEvaluator,
    constants_env: &Env,
    language: Language,
) -> HashMap<String, Env> {
    let mut captured_scopes: HashMap<String, Env> = HashMap::new();

    for (name, ast) in callables {
        if ast.nested.is_empty() {
            continue;
        }
        let mangled = mangle_callable_name(name, language);
        let Ok(result) = symbolic_evaluation_with_env(
            callables_map,
            &mangled,
            evaluator.matcher(),
            constants_env,
        ) else {
            continue;
        };

        for nested_ref in &ast.nested {
            let captured: Env = nested_ref
                .captured
                .iter()
                .filter_map(|var| result.final_env.get(var).map(|v| (var.clone(), v.clone())))
                .filter(|(_, (_, expr))| *expr != Expr::Empty)
                .collect();

            if !captured.is_empty() {
                // Key by hash (unique per function body) so sibling inner functions
                // with the same mangled name get distinct entries. Matches the
                // lookup key `restcall.function_hash` used above.
                captured_scopes
                    .entry(nested_ref.hash.clone())
                    .or_insert(captured);
            }
        }
    }

    captured_scopes
}
