use std::collections::HashMap;

use log::info;
use models::{
    ParsedCallable, RestCall,
    ir::{
        ast::{CallableAst, Expr},
        language::Language,
        project::{ImportKind, ProjectIR, TypedFileRecord},
    },
};
use statix::symbolic_evaluation_with_env;

use crate::pipeline::{
    pass2::callables::mangle_callable_name,
    pass3::{
        constants::constants_to_env, language_backend::LanguageSpecificEvaluator,
        pass_module::PerFileModuleConsts,
    },
};

use super::callables::{build_file_local_callables, build_merged_enums};
use super::language_backend::evaluation_for;

type Env = HashMap<String, (Option<String>, Expr)>;

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

fn build_file_env(
    file: &TypedFileRecord,
    project_ir: &ProjectIR,
    constants_env: &Env,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
    per_file_module_consts: &PerFileModuleConsts,
) -> Env {
    let mut env: Env = constants_env.clone();

    if let Some(attrs) = per_file_attrs.get(&file.file_path) {
        for (k, v) in attrs {
            env.entry(k.clone())
                .or_insert_with(|| (Some("String".to_string()), Expr::Literal(v.clone())));
        }
    }

    // Same-file module globals (e.g. `aaa_url = settings.x`) are visible to
    // every function in the file without an explicit import.
    if let Some(own_consts) = per_file_module_consts.get(&file.file_path) {
        for (name, value) in own_consts {
            env.entry(name.clone())
                .or_insert_with(|| (Some("String".to_string()), Expr::Literal(value.clone())));
        }
    }

    // Propagate module-level constants imported from other files. Only
    // `ImportKind::Constant` entries are considered; keying by `import.codeword`
    // preserves `from m import x as y` aliases.
    for import in &file.imports {
        let Some(resolved) = project_ir
            .import_graph
            .lookup(&file.file_path, &import.codeword)
        else {
            continue;
        };
        if !matches!(resolved.kind, ImportKind::Constant) {
            continue;
        }
        let Some(source_consts) = per_file_module_consts.get(&resolved.source_file) else {
            continue;
        };
        let Some(value) = source_consts.get(&resolved.fully_qualified_name) else {
            continue;
        };
        env.entry(import.codeword.clone())
            .or_insert_with(|| (Some("String".to_string()), Expr::Literal(value.clone())));
    }

    env
}
