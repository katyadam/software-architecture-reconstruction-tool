use std::collections::HashMap;

use models::{
    Scope,
    ir::{
        ast::Expr,
        project::{ConstantValue, ImportGraph, ImportKind, ProjectIR, TypedFileRecord},
    },
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

    // Own globals shadow a same-named constant from another file. The harvest below
    // rebinds evaluated values but drops `""`, so a quoted literal is kept here.
    for (key, assignment) in &file.assignments {
        let name = &key.variable_name;
        let foreign = project_ir
            .constants
            .get(name)
            .is_some_and(|c| c.source_file != file.file_path);
        if !foreign || !matches!(key.scope, Scope::Global) {
            continue;
        }
        match unquote(assignment.value.trim()) {
            Some(value) => bind_local(&mut env, &project_ir.constants, name, value),
            None => {
                env.remove(name);
            }
        }
    }

    // Same-file module globals (e.g. `aaa_url = settings.x`) are visible to
    // every function in the file without an explicit import.
    if let Some(own_consts) = per_file_module_consts.get(&file.file_path) {
        for (name, value) in own_consts {
            bind_local(&mut env, &project_ir.constants, name, value);
        }
    }

    fill_env_with_imported_constants(
        &mut env,
        file,
        &project_ir.import_graph,
        &project_ir.constants,
        per_file_module_consts,
        per_file_attrs,
    );

    env
}

/// File-local binding: overwrites a project constant, but not an external-only name.
fn bind_local(env: &mut Env, constants: &HashMap<String, ConstantValue>, name: &str, value: &str) {
    let bound = (Some("String".to_string()), Expr::Literal(value.to_string()));
    if constants.contains_key(name) {
        env.insert(name.to_string(), bound);
    } else {
        env.entry(name.to_string()).or_insert(bound);
    }
}

/// Content of a `"..."`, `'...'` or `` `...` `` literal; `"a" + x + "b"` is not one.
pub(super) fn unquote(expr: &str) -> Option<&str> {
    let quote = expr
        .chars()
        .next()
        .filter(|c| matches!(c, '"' | '\'' | '`'))?;
    expr.strip_prefix(quote)?
        .strip_suffix(quote)
        .filter(|inner| !inner.contains(quote))
}

/// Propagate constants imported from other files. Only `ImportKind::Constant` entries
/// are considered; keying by `import.codeword` preserves `from m import x as y` aliases.
/// Scalars overwrite same-named project constants; attrs only fill gaps.
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
    constants: &HashMap<String, ConstantValue>,
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
            bind_local(env, constants, &import.codeword, value);
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use models::{
        Import,
        ir::{
            ast::Expr,
            language::Language,
            project::{
                ClassHierarchy, ConstantValue, ImportGraph, ImportKind, ProjectIR, ResolvedImport,
                TypedFileRecord,
            },
        },
    };

    use super::build_file_env;

    fn rec(path: &str) -> TypedFileRecord {
        TypedFileRecord {
            file_path: path.to_string(),
            language: Language::Python,
            imports: vec![],
            entities: vec![],
            endpoints: vec![],
            callables: vec![],
            call_statements: vec![],
            assignments: HashMap::new(),
            enums: vec![],
            raw_restcalls: vec![],
            raw_message_edges: vec![],
        }
    }

    fn consts(entries: &[(&str, &str, &str)]) -> HashMap<String, HashMap<String, String>> {
        let mut out: HashMap<String, HashMap<String, String>> = HashMap::new();
        for (file, name, value) in entries {
            out.entry(file.to_string())
                .or_default()
                .insert(name.to_string(), value.to_string());
        }
        out
    }

    fn value(env: &super::Env, key: &str) -> String {
        match &env[key].1 {
            Expr::Literal(v) => v.clone(),
            other => panic!("{key}: not a literal: {other:?}"),
        }
    }

    #[test]
    fn file_bindings_beat_project_constants_attrs_only_fill_gaps() {
        let mut importer = rec("c/routes.py");
        importer.imports.push(Import::from_parts(
            "config",
            "BASE_PATH",
            "",
            "",
            "BASE_PATH",
        ));
        let ir = ProjectIR {
            files: vec![rec("a/routes.py"), rec("b/routes.py"), importer],
            import_graph: ImportGraph {
                resolved_imports: HashMap::from([(
                    ("c/routes.py".to_string(), "BASE_PATH".to_string()),
                    ResolvedImport {
                        source_file: "c/config.py".to_string(),
                        fully_qualified_name: "BASE_PATH".to_string(),
                        kind: ImportKind::Constant,
                    },
                )]),
            },
            class_hierarchy: ClassHierarchy {
                parents: HashMap::new(),
                children: HashMap::new(),
            },
            constants: HashMap::from([(
                "BASE_PATH".to_string(),
                ConstantValue {
                    name: "BASE_PATH".to_string(),
                    value: "\"/a\"".to_string(),
                    source_file: "a/routes.py".to_string(),
                },
            )]),
            callable_map: HashMap::new(),
            callables_by_file_hash: HashMap::new(),
        };
        let constants_env = super::build_constants_env(&ir.constants, &HashMap::new());
        // Harvest includes the seeded project value for files that do not rebind it.
        let module_consts = consts(&[
            ("a/routes.py", "BASE_PATH", "/a"),
            ("b/routes.py", "BASE_PATH", "/b"),
            ("c/routes.py", "BASE_PATH", "/a"),
            ("c/config.py", "BASE_PATH", "/c"),
        ]);
        let attrs = consts(&[("a/routes.py", "BASE_PATH", "/attr")]);
        let env_of = |i: usize| {
            super::build_file_env(&ir.files[i], &ir, &constants_env, &attrs, &module_consts)
        };
        assert_eq!(value(&env_of(0), "BASE_PATH"), "/a");
        assert_eq!(value(&env_of(1), "BASE_PATH"), "/b");
        assert_eq!(value(&env_of(2), "BASE_PATH"), "/c");

        // Attrs alone do not override a project constant.
        let no_module = HashMap::new();
        let env = build_file_env(&ir.files[0], &ir, &constants_env, &attrs, &no_module);
        assert_eq!(value(&env, "BASE_PATH"), "/a");
    }
}
