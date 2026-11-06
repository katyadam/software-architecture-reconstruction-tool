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
    ParsedCallable, Scope,
    ir::{
        ast::{CallableAst, Expr},
        project::{ImportKind, ProjectIR, TypedFileRecord},
    },
};
use statix::symbolic_evaluation_with_env;

use crate::pipeline::{
    pass2::callables::mangle_callable_name,
    pass3::{
        callables::build_file_local_callables, env::build_constants_env,
        language_backend::evaluation_for, pass_attr::PerFileAttrMap,
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
/// 4. Second round only: values of the file's imported constants.
///
/// A project constant the file rebinds is not seeded, so an unresolvable rebinding
/// drops the name instead of harvesting another file's value.
///
/// Files whose language has no synthetic `<module>` callable (or whose module
/// body produces no literals) simply contribute no entry to the result.
/// TODO: currently this is a huge bottleneck - running symbolic evaluation on each file, and then on each restcall
pub fn resolve_all(
    project_ir: &ProjectIR,
    external_constants: &HashMap<String, String>,
    per_file_attrs: &PerFileAttrMap,
) -> PerFileModuleConsts {
    let base_env = build_constants_env(&project_ir.constants, external_constants);

    let mut result = PerFileModuleConsts::new();

    for file in &project_ir.files {
        let Some(literals) = evaluate_module(
            file,
            project_ir,
            &base_env,
            per_file_attrs.get(&file.file_path),
            &HashMap::new(),
        ) else {
            continue;
        };

        if !literals.is_empty() {
            result.insert(file.file_path.clone(), literals);
        }
    }

    // ponytail: one round read from a snapshot, a re-export of a re-export sees round-one values.
    let second_round: PerFileModuleConsts = project_ir
        .files
        .iter()
        .filter_map(|file| {
            let imported =
                imported_constants(file, project_ir, external_constants, &base_env, &result);
            if imported.is_empty() {
                return None;
            }
            let literals = evaluate_module(
                file,
                project_ir,
                &base_env,
                per_file_attrs.get(&file.file_path),
                &imported,
            )?;
            Some((file.file_path.clone(), literals))
        })
        .collect();
    result.extend(second_round);

    result
}

/// Imported constants whose source value differs from the seed; external-only names keep priority.
fn imported_constants(
    file: &TypedFileRecord,
    project_ir: &ProjectIR,
    external_constants: &HashMap<String, String>,
    base_env: &Env,
    harvested: &PerFileModuleConsts,
) -> HashMap<String, String> {
    file.imports
        .iter()
        .filter_map(|import| {
            let name = &import.codeword;
            let resolved = project_ir.import_graph.lookup(&file.file_path, name)?;
            if !matches!(resolved.kind, ImportKind::Constant) {
                return None;
            }
            if external_constants.contains_key(name) && !project_ir.constants.contains_key(name) {
                return None;
            }
            let value = harvested
                .get(&resolved.source_file)?
                .get(&resolved.fully_qualified_name)?;
            let seeded = matches!(base_env.get(name), Some((_, Expr::Literal(v))) if v == value);
            (!seeded).then(|| (name.clone(), value.clone()))
        })
        .collect()
}

/// Returns `None` if the file has no `<module>` callable (e.g. Java).
fn evaluate_module(
    file: &TypedFileRecord,
    project_ir: &ProjectIR,
    base_env: &Env,
    file_attrs: Option<&HashMap<String, String>>,
    imported: &HashMap<String, String>,
) -> Option<HashMap<String, String>> {
    let mut callables = build_file_local_callables(file, &project_ir.callable_map);
    let evaluator = evaluation_for(file.language)?;
    let mangled = mangle_callable_name(MODULE_CALLABLE_NAME, file.language);

    let module_pc = callables.remove(&mangled)?;
    let mut seed_env = build_seed_env(base_env, file_attrs);
    for key in file.assignments.keys() {
        if matches!(key.scope, Scope::Global)
            && project_ir.constants.contains_key(&key.variable_name)
        {
            seed_env.remove(&key.variable_name);
        }
    }
    for (name, value) in imported {
        seed_env.insert(
            name.clone(),
            (Some("String".to_string()), Expr::Literal(value.clone())),
        );
    }
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
