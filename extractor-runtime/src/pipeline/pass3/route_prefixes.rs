//! Evaluate the prefix chains recorded by pass 2 and apply them to endpoint URIs.

use std::collections::{BTreeSet, HashMap, HashSet};

use models::{
    AssignmentKey, Endpoint, Scope,
    endpoints::PrefixExpr,
    ir::{
        ast::Expr,
        language::Language,
        project::{ImportKind, ProjectIR, TypedFileRecord},
    },
};

use crate::pipeline::pass3::{
    env::{Env, build_constants_env, build_file_env, unquote},
    pass_module::PerFileModuleConsts,
};

/// Endpoints of all files, one per distinct resolved prefix.
pub(super) fn evaluate_endpoints(
    project_ir: &ProjectIR,
    external_constants: &HashMap<String, String>,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
    per_file_module_consts: &PerFileModuleConsts,
) -> Vec<Endpoint> {
    let endpoints = || project_ir.files.iter().flat_map(|f| &f.endpoints);
    let used: HashSet<&str> = endpoints()
        .flat_map(|e| e.route_prefixes.iter().flatten())
        .map(|p| p.file_path.as_str())
        .collect();
    let constants_env = build_constants_env(&project_ir.constants, external_constants);
    let declared = declared_values(project_ir);
    let by_path: HashMap<&str, &TypedFileRecord> = project_ir
        .files
        .iter()
        .map(|f| (f.file_path.as_str(), f))
        .collect();
    let envs: HashMap<&str, Env> = project_ir
        .files
        .iter()
        .filter(|f| used.contains(f.file_path.as_str()))
        .map(|f| {
            let env = build_file_env(
                f,
                project_ir,
                &constants_env,
                per_file_attrs,
                per_file_module_consts,
            );
            let env = prefix_env(
                env,
                f,
                project_ir,
                per_file_module_consts,
                &declared,
                &by_path,
            );
            (f.file_path.as_str(), env)
        })
        .collect();
    endpoints().flat_map(|e| expand(e, &envs)).collect()
}

/// Every project-wide declared value per constant, same rule as `collect_constants`.
fn declared_values(project_ir: &ProjectIR) -> HashMap<&str, HashSet<&str>> {
    let mut declared: HashMap<&str, HashSet<&str>> = HashMap::new();
    for file in &project_ir.files {
        for (key, assignment) in &file.assignments {
            if matches!(key.scope, Scope::Global | Scope::Class(_))
                && project_ir.constants.contains_key(&key.variable_name)
            {
                declared
                    .entry(&key.variable_name)
                    .or_default()
                    .insert(assignment.value.trim().trim_matches(['"', '\'']));
            }
        }
    }
    declared
}

/// Global assignment of `name` in `file`.
fn global<'a>(file: &'a TypedFileRecord, name: &str) -> Option<&'a str> {
    file.assignments
        .get(&AssignmentKey {
            scope: Scope::Global,
            variable_name: name.to_string(),
        })
        .map(|a| a.value.as_str())
}

/// File env narrowed to values a prefix may use: no source text, no name that only
/// another file's declaration could bind. REST call evaluation keeps the plain env.
fn prefix_env(
    mut env: Env,
    file: &TypedFileRecord,
    project_ir: &ProjectIR,
    module_consts: &PerFileModuleConsts,
    declared: &HashMap<&str, HashSet<&str>>,
    by_path: &HashMap<&str, &TypedFileRecord>,
) -> Env {
    let is_literal_empty = |raw: &str| unquote(raw.trim()) == Some("");
    let empty = || (Some("String".to_string()), Expr::Literal(String::new()));
    // The harvest drops `""`: restore own and imported ones.
    for (key, assignment) in &file.assignments {
        if matches!(key.scope, Scope::Global) && is_literal_empty(&assignment.value) {
            env.insert(key.variable_name.clone(), empty());
        }
    }
    for import in &file.imports {
        let word = &import.codeword;
        let raw = project_ir
            .import_graph
            .lookup(&file.file_path, word)
            .filter(|r| matches!(r.kind, ImportKind::Constant))
            .and_then(|r| {
                global(
                    by_path.get(r.source_file.as_str())?,
                    &r.fully_qualified_name,
                )
            });
        if global(file, word).is_none() && raw.is_some_and(is_literal_empty) {
            env.insert(word.clone(), empty());
        }
    }
    env.retain(|name, (_, value)| {
        if let Some(raw) = global(file, name) {
            return !is_source_text(raw, value, file.language);
        }
        let constant = project_ir.constants.get(name);
        let import = project_ir
            .import_graph
            .lookup(&file.file_path, name)
            .filter(|r| matches!(r.kind, ImportKind::Constant));
        if let Some(import) = import {
            let source = by_path.get(import.source_file.as_str());
            let raw = source.and_then(|f| global(f, &import.fully_qualified_name));
            if raw.is_some_and(is_literal_empty) {
                return true;
            }
            let harvested = module_consts
                .get(&import.source_file)
                .is_some_and(|c| c.contains_key(&import.fully_qualified_name));
            // Not harvested: `value` is a same-named constant from elsewhere.
            return harvested || constant.is_none();
        }
        let Some(constant) = constant else {
            return true;
        };
        let unique = declared
            .get(name.as_str())
            .is_some_and(|values| values.len() == 1);
        let language = by_path
            .get(constant.source_file.as_str())
            .map_or(file.language, |f| f.language);
        unique && !is_source_text(&constant.value, value, language)
    });
    env
}

fn expand(endpoint: &Endpoint, envs: &HashMap<&str, Env>) -> Vec<Endpoint> {
    let mut prefixes: BTreeSet<String> = endpoint
        .route_prefixes
        .iter()
        .map(|chain| resolve_chain(chain, envs))
        .collect();
    if prefixes.is_empty() {
        prefixes.insert(String::new());
    }
    let mut base = endpoint.clone();
    base.route_prefixes.clear();
    prefixes
        .into_iter()
        .map(|prefix| {
            let mut clone = base.clone();
            if !prefix.is_empty() {
                clone.uri = join(&prefix, &endpoint.uri);
            }
            clone
        })
        .collect()
}

/// Joined chain; `""` (keep the declared URI) if any expression is unresolved.
fn resolve_chain(chain: &[PrefixExpr], envs: &HashMap<&str, Env>) -> String {
    chain
        .iter()
        .try_fold(String::new(), |prefix, part| {
            let env = envs.get(part.file_path.as_str())?;
            Some(join(&prefix, &eval_prefix(&part.expr, env)?))
        })
        .unwrap_or_default()
}

/// String value of one prefix expression: quoted literal or exact env key.
fn eval_prefix(expr: &str, env: &Env) -> Option<String> {
    let expr = expr.trim();
    if let Some(literal) = unquote(expr) {
        return Some(literal.to_string());
    }
    match env.get(expr) {
        Some((_, Expr::Literal(value))) if !is_raw_fstring(value) => Some(value.clone()),
        _ => None,
    }
}

/// `value` is still the source text of `raw` (`BASE = settings.base`), not a string value.
/// Go stores string constants unquoted, so a path-like Go value counts as a string.
fn is_source_text(raw: &str, value: &Expr, language: Language) -> bool {
    let Expr::Literal(value) = value else {
        return false;
    };
    let raw = raw.trim();
    if matches!(language, Language::Go) && value.starts_with('/') {
        return false;
    }
    if is_raw_fstring(raw) {
        return value.contains('{');
    }
    unquote(raw).is_none() && raw.trim_matches(['"', '\'']) == value
}

fn is_raw_fstring(value: &str) -> bool {
    value.starts_with("f\"") || value.starts_with("f'")
}

/// `a` + `/` + `b`, empty parts skipped, leading `/`.
fn join(a: &str, b: &str) -> String {
    let (a, b) = (a.trim_end_matches('/'), b.trim_start_matches('/'));
    let joined = match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a}/{b}"),
    };
    if joined.is_empty() || joined.starts_with('/') {
        joined
    } else {
        format!("/{joined}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use models::{
        Endpoint,
        endpoints::PrefixExpr,
        ir::{
            ast::Expr,
            language::Language,
            project::{
                ClassHierarchy, ConstantValue, ImportGraph, ImportKind, ProjectIR, ResolvedImport,
                TypedFileRecord,
            },
        },
    };

    use super::{Env, eval_prefix, evaluate_endpoints};
    use crate::pipeline::pass3::pass_module::PerFileModuleConsts;

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

    fn ep(uri: &str, chains: &[&[(&str, &str)]]) -> Endpoint {
        Endpoint {
            uri: uri.to_string(),
            route_prefixes: chains
                .iter()
                .map(|chain| {
                    chain
                        .iter()
                        .map(|(file, expr)| PrefixExpr {
                            file_path: file.to_string(),
                            expr: expr.to_string(),
                        })
                        .collect()
                })
                .collect(),
            ..Default::default()
        }
    }

    /// `constants` are declared as globals of the first file.
    fn project(mut files: Vec<TypedFileRecord>, constants: &[(&str, &str)]) -> ProjectIR {
        for (name, value) in constants {
            files[0].assignments.insert(
                models::AssignmentKey {
                    scope: models::Scope::Global,
                    variable_name: name.to_string(),
                },
                models::Assignment {
                    variable_name: name.to_string(),
                    variable_type: String::new(),
                    value: value.to_string(),
                },
            );
        }
        ProjectIR {
            files,
            import_graph: ImportGraph {
                resolved_imports: HashMap::new(),
            },
            class_hierarchy: ClassHierarchy {
                parents: HashMap::new(),
                children: HashMap::new(),
            },
            constants: constants
                .iter()
                .map(|(name, value)| {
                    let constant = ConstantValue {
                        name: name.to_string(),
                        value: value.to_string(),
                        source_file: String::new(),
                    };
                    (name.to_string(), constant)
                })
                .collect(),
            callable_map: HashMap::new(),
            callables_by_file_hash: HashMap::new(),
        }
    }

    fn uris(ir: &ProjectIR, module_consts: &PerFileModuleConsts) -> Vec<String> {
        let mut out: Vec<String> =
            evaluate_endpoints(ir, &HashMap::new(), &HashMap::new(), module_consts)
                .into_iter()
                .map(|e| e.uri)
                .collect();
        out.sort();
        out
    }

    fn env(entries: &[(&str, Expr)]) -> Env {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), (None, v.clone())))
            .collect()
    }

    #[test]
    fn eval_quoted_literals() {
        let empty = env(&[]);
        for expr in ["\"/api\"", "'/api'", "`/api`", " \"/api\" "] {
            assert_eq!(eval_prefix(expr, &empty).as_deref(), Some("/api"), "{expr}");
        }
    }

    #[test]
    fn eval_env_literal_by_exact_key() {
        let env = env(&[
            ("BASE", Expr::Literal("/chained".to_string())),
            ("settings.base", Expr::Literal("/external".to_string())),
        ]);
        assert_eq!(eval_prefix("BASE", &env).as_deref(), Some("/chained"));
        assert_eq!(
            eval_prefix("settings.base", &env).as_deref(),
            Some("/external")
        );
    }

    #[test]
    fn eval_rejects_unknown_raw_fstring_and_non_literal() {
        let env = env(&[
            ("RAW", Expr::Literal("f\"/x/{Y}\"".to_string())),
            ("NOT_LITERAL", Expr::Var("y".to_string())),
        ]);
        for expr in ["unknown", "RAW", "NOT_LITERAL", "ROOT + \"/x\""] {
            assert_eq!(eval_prefix(expr, &env), None, "{expr}");
        }
    }

    #[test]
    fn prefix_from_global_constant() {
        let mut f = rec("svc/routes.py");
        f.endpoints
            .push(ep("/consigns", &[&[("svc/routes.py", "BASE_PATH")]]));
        let ir = project(vec![f], &[("BASE_PATH", "\"/api/v1/consignservice\"")]);
        assert_eq!(
            uris(&ir, &HashMap::new()),
            ["/api/v1/consignservice/consigns"]
        );
    }

    #[test]
    fn prefix_from_imported_constant() {
        let mut routes = rec("svc/routes.py");
        routes.imports.push(models::Import::from_parts(
            "config",
            "BASE_PATH",
            "",
            "",
            "BASE_PATH",
        ));
        routes
            .endpoints
            .push(ep("/consigns", &[&[("svc/routes.py", "BASE_PATH")]]));
        let mut ir = project(vec![routes, rec("svc/config.py")], &[]);
        ir.import_graph.resolved_imports.insert(
            ("svc/routes.py".to_string(), "BASE_PATH".to_string()),
            ResolvedImport {
                source_file: "svc/config.py".to_string(),
                fully_qualified_name: "BASE_PATH".to_string(),
                kind: ImportKind::Constant,
            },
        );
        let module_consts = HashMap::from([(
            "svc/config.py".to_string(),
            HashMap::from([(
                "BASE_PATH".to_string(),
                "/api/v1/consignservice".to_string(),
            )]),
        )]);
        assert_eq!(
            uris(&ir, &module_consts),
            ["/api/v1/consignservice/consigns"]
        );
    }

    #[test]
    fn helper_reused_by_two_mounts_yields_two_endpoints() {
        let mut f = rec("svc/routes.py");
        f.endpoints.push(ep(
            "/alive",
            &[&[("svc/app.py", "\"/v1\"")], &[("svc/app.py", "\"/v3\"")]],
        ));
        let ir = project(vec![f, rec("svc/app.py")], &[]);
        assert_eq!(uris(&ir, &HashMap::new()), ["/v1/alive", "/v3/alive"]);
    }

    #[test]
    fn unresolved_prefix_keeps_uri() {
        let mut f = rec("svc/app.py");
        f.endpoints.push(ep(
            "/x",
            &[&[("svc/app.py", "\"/api\""), ("svc/app.py", "settings.base")]],
        ));
        let ir = project(vec![f], &[]);
        assert_eq!(uris(&ir, &HashMap::new()), ["/x"]);
    }

    #[test]
    fn chains_with_same_value_collapse() {
        let mut f = rec("svc/app.py");
        f.endpoints.push(ep(
            "/x",
            &[&[("svc/app.py", "\"/v1\"")], &[("svc/app.py", "V1")]],
        ));
        let ir = project(vec![f], &[("V1", "\"/v1\"")]);
        assert_eq!(uris(&ir, &HashMap::new()), ["/v1/x"]);
    }

    #[test]
    fn no_chains_or_empty_chain_keep_uri() {
        let mut f = rec("svc/app.py");
        f.endpoints.push(ep("/a", &[]));
        f.endpoints.push(ep("/b", &[&[]]));
        let ir = project(vec![f], &[]);
        assert_eq!(uris(&ir, &HashMap::new()), ["/a", "/b"]);
    }
}
