//! Pass-module: propagate module-level assignments across files.
//!
//! Every Python file carries a synthetic `<module>` callable whose body is the
//! sequence of top-level statements. This pass runs `symbolic_evaluation_with_env`
//! on that callable (seeded with project constants, CLI overrides, and per-file
//! attribute defaults) and harvests any variable in the final env that reduced to
//! an `Expr::Literal`. Pass 3 then uses the per-file map to materialise imported
//! module constants in consumer files' evaluation environments.

use std::collections::HashMap;

use models::{
    ParsedCallable,
    ir::{
        ast::{CallableAst, Expr},
        project::ProjectIR,
    },
};
use statix::symbolic_evaluation_with_env;

use crate::pipeline::{
    pass_attr::PerFileAttrMap,
    pass3::{
        callables::{build_file_local_callables, build_project_global_callables, constants_to_env},
        language_backend::{evaluation_for, mangle_callable_name},
    },
};

/// Per-file module-const map: file path -> (variable name -> stripped literal).
pub type PerFileModuleConsts = HashMap<String, HashMap<String, String>>;

/// Synthetic callable name emitted by the Python extractor for the module
/// body. Angle brackets keep it distinct from any real identifier so it
/// cannot collide with a user-defined function, and let `pass3::evaluate`
/// filter it out of `EvaluatedIR.callables` before synthesis consumes it.
pub const MODULE_CALLABLE_NAME: &str = "<module>()";

type Env = HashMap<String, (Option<String>, Expr)>;

/// Evaluate every file's `<module>` callable and collect the literal-valued
/// bindings that survive.
///
/// The initial env for each file is:
/// 1. Project constants (from Pass 2).
/// 2. CLI `external_constants` (only fill gaps).
/// 3. Per-file attribute defaults from `pass_attr::resolve_all`.
///
/// Files whose language has no synthetic `<module>` callable (or whose module
/// body produces no literals) simply contribute no entry to the result.
/// TODO: currently this is a huge bottleneck - running symbolic evaluation on each file, and then on each restcall
pub fn resolve_all(
    project_ir: &ProjectIR,
    external_constants: &HashMap<String, String>,
    per_file_attrs: &PerFileAttrMap,
) -> PerFileModuleConsts {
    let global_callables = build_project_global_callables(&project_ir.files);
    let base_env = build_base_env(project_ir, external_constants);

    let mut result = PerFileModuleConsts::new();

    for file in &project_ir.files {
        let Some(literals) = evaluate_module(
            file,
            &global_callables,
            &base_env,
            per_file_attrs.get(&file.file_path),
        ) else {
            continue;
        };

        if !literals.is_empty() {
            result.insert(file.file_path.clone(), literals);
        }
    }

    result
}

/// Build the shared base env: project constants layered above CLI externals.
fn build_base_env(project_ir: &ProjectIR, external_constants: &HashMap<String, String>) -> Env {
    let mut env = constants_to_env(&project_ir.constants);
    for (name, value) in external_constants {
        let trimmed = value
            .trim_matches(|c: char| c == '"' || c == '\'')
            .to_string();
        env.entry(name.clone())
            .or_insert_with(|| (Some("String".to_string()), Expr::Literal(trimmed)));
    }
    env
}

/// Returns `None` if the file has no `<module>` callable (e.g. Java).
fn evaluate_module(
    file: &models::ir::project::TypedFileRecord,
    global_callables: &HashMap<String, ParsedCallable>,
    base_env: &Env,
    file_attrs: Option<&HashMap<String, String>>,
) -> Option<HashMap<String, String>> {
    let mut callables = build_file_local_callables(file, global_callables);
    let evaluator = evaluation_for(file.language);
    let mangled = mangle_callable_name(MODULE_CALLABLE_NAME, file.language);

    let module_pc = callables.remove(&mangled)?;
    let seed_env = build_seed_env(base_env, file_attrs);
    let final_env = run_statements(&mut callables, &mangled, module_pc, evaluator, seed_env);
    Some(harvest_literals(final_env))
}

/// Layer per-file attribute defaults on top of the shared base env.
fn build_seed_env(base_env: &Env, file_attrs: Option<&HashMap<String, String>>) -> Env {
    let mut env = base_env.clone();
    if let Some(attrs) = file_attrs {
        for (k, v) in attrs {
            env.entry(k.clone())
                .or_insert_with(|| (Some("String".to_string()), Expr::Literal(v.clone())));
        }
    }
    env
}

/// Evaluate each statement in isolation, which is achieved by having a map with single key-val
/// that is updated for every statement. Successful bindings are continously accumulated.
/// Errors on individual statements are skipped so one unresolvable RHS
/// (e.g. `logger = unknown_fn()`) does not discard earlier results.
/// TODO: Create a better solution to handle errors in symbolic analysis,
/// so symbolic evaluation can evaluate statements on module-level together without halting on error
fn run_statements(
    callables: &mut HashMap<String, ParsedCallable>,
    mangled: &str,
    module_pc: ParsedCallable,
    evaluator: &dyn crate::pipeline::pass3::language_backend::LanguageSpecificEvaluator,
    mut env: Env,
) -> Env {
    for stmt in module_pc.ast.statements {
        callables.insert(
            mangled.to_string(),
            ParsedCallable {
                metadata: module_pc.metadata.clone(),
                ast: CallableAst {
                    statements: vec![stmt],
                    nested: vec![],
                },
            },
        );
        if let Ok(analysis) =
            symbolic_evaluation_with_env(callables, mangled, evaluator.matcher(), &env)
        {
            env = analysis.final_env;
        }
    }
    env
}

/// Extract non-empty `Expr::Literal` values from the env.
fn harvest_literals(env: Env) -> HashMap<String, String> {
    env.into_iter()
        .filter_map(|(name, (_, expr))| match expr {
            Expr::Literal(value) if !value.is_empty() => Some((name, value)),
            _ => None,
        })
        .collect()
}
