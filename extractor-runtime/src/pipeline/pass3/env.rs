use std::collections::HashMap;

use models::ir::{
    ast::Expr,
    project::{ImportGraph, ImportKind, ProjectIR, TypedFileRecord},
};

use crate::pipeline::pass3::{constants::constants_to_env, pass_module::PerFileModuleConsts};

pub(super) type Env = HashMap<String, (Option<String>, Expr)>;

pub(super) fn build_file_env(
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

    fill_env_with_imported_constants(
        &mut env,
        file,
        &project_ir.import_graph,
        per_file_module_consts,
        per_file_attrs,
    );

    env
}

/// Propagate constants imported from other files. Only `ImportKind::Constant` entries
/// are considered; keying by `import.codeword` preserves `from m import x as y` aliases.
///
/// Two injections per resolved import:
/// 1. Scalar string constant: `from m import base_url` -> inject `base_url = "http://..."`.
/// 2. Object-instance attrs: `from m import settings` -> inject `settings.as_url = "aaaa"`
///    for every dotted attr default in per_file_attrs for the source file.
///    Instances are indexed as Constant (global assignments), not Entity (class defs).
fn fill_env_with_imported_constants(
    env: &mut Env,
    file: &TypedFileRecord,
    import_graph: &ImportGraph,
    per_file_module_consts: &PerFileModuleConsts,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
) {
    for import in &file.imports {
        let Some(resolved) = import_graph.lookup(&file.file_path, &import.codeword) else {
            continue;
        };
        if !matches!(resolved.kind, ImportKind::Constant) {
            continue;
        }

        if let Some(source_consts) = per_file_module_consts.get(&resolved.source_file)
            && let Some(value) = source_consts.get(&resolved.fully_qualified_name)
        {
            env.entry(import.codeword.clone())
                .or_insert_with(|| (Some("String".to_string()), Expr::Literal(value.clone())));
        }

        if let Some(source_attrs) = per_file_attrs.get(&resolved.source_file) {
            let prefix = format!("{}.", import.codeword);
            for (key, value) in source_attrs {
                if key.starts_with(&prefix) {
                    env.entry(key.clone()).or_insert_with(|| {
                        (Some("String".to_string()), Expr::Literal(value.clone()))
                    });
                }
            }
        }
    }
}

/// Merge project-scanned constants with CLI-supplied external constants into one env.
/// External constants are lower priority — they don't overwrite project-scanned values.
/// Dotted-path keys like `"settings.as_url"` are preserved as-is.
pub(super) fn build_constants_env(
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
