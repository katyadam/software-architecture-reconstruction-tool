use std::collections::{BTreeMap, BTreeSet};

use log::warn;
use models::{
    Parameter, RouteFact, Scope,
    endpoints::PrefixExpr,
    ir::{
        language::Language,
        project::{ImportGraph, TypedFileRecord},
    },
};

/// Max call/attach chain followed per endpoint.
const MAX_DEPTH: usize = 8;
/// Max chains kept per router; non-literal prefixes do not collapse before pass 3.
// ponytail: smallest chains kept, wider routers lose mounts; raise if a real project hits the warning.
const MAX_CHAINS: usize = 256;

/// Record on every endpoint the unevaluated prefix chains of its router.
pub fn resolve_route_prefixes(files: &mut [TypedFileRecord], import_graph: &ImportGraph) {
    let facts: Vec<(usize, Vec<RouteFact>)> = files
        .iter()
        .enumerate()
        .map(|(i, file)| (i, facts_for(file)))
        .filter(|(_, facts)| !facts.is_empty())
        .collect();
    resolve_with_facts(files, &facts, import_graph);
}

/// Per-language route fact adapters.
fn facts_for(file: &TypedFileRecord) -> Vec<RouteFact> {
    match file.language {
        Language::Python => python_extractor::extraction::route_facts::route_facts(file),
        Language::Go => go_extractor::extraction::route_facts::route_facts(file),
        _ => vec![],
    }
}

/// Bare function name: extractors may store `name(params) -> ret`
/// or a Go signature `func (r *R) name[T any](params) ret`.
fn bare(name: &str) -> &str {
    let name = name.trim();
    let Some(go) = name.strip_prefix("func ") else {
        return name.split('(').next().unwrap_or(name).trim();
    };
    let go = go.trim_start();
    let go = match go.strip_prefix('(') {
        Some(receiver) => receiver.split_once(')').map_or(go, |(_, rest)| rest),
        None => go,
    };
    go.split(['(', '[']).next().unwrap_or(go).trim()
}

/// Scope key: bare function/class name, `""` for module level.
fn scope_key(scope: &Scope) -> String {
    match scope {
        Scope::Global => String::new(),
        Scope::Function(name) | Scope::Class(name) => bare(name).to_string(),
    }
}

/// A call site: caller file, caller scope key, call arguments.
struct Site<'a> {
    file: usize,
    scope: String,
    args: &'a [models::Argument],
}

/// `(file, scope key, variable)`.
type VarKey = (usize, String, String);

/// Prefix chains, outermost first; an empty chain is a root router.
type Prefixes = BTreeSet<Vec<PrefixExpr>>;

/// Add `chains`, keeping the first `MAX_CHAINS` in set order.
fn merge(set: &mut Prefixes, chains: impl IntoIterator<Item = Vec<PrefixExpr>>) {
    for chain in chains {
        set.insert(chain);
        if set.len() > MAX_CHAINS {
            set.pop_last();
        }
    }
}

struct Resolver<'a> {
    files: &'a [TypedFileRecord],
    graph: &'a ImportGraph,
    by_path: BTreeMap<&'a str, usize>,
    /// `(file, scope, child)` -> `(parent, prefix)` of `Attach` facts.
    attach: BTreeMap<VarKey, Vec<(&'a str, &'a str)>>,
    /// `(file, scope, router)` -> own prefixes.
    own: BTreeMap<VarKey, Vec<&'a str>>,
    /// Callee last name segment -> call sites.
    calls: BTreeMap<&'a str, Vec<Site<'a>>>,
    /// Callable bare name -> `(file, parameters)`.
    callables: BTreeMap<&'a str, Vec<(usize, &'a [Parameter])>>,
}

impl<'a> Resolver<'a> {
    fn new(
        files: &'a [TypedFileRecord],
        facts: &'a [(usize, Vec<RouteFact>)],
        graph: &'a ImportGraph,
    ) -> Self {
        let mut r = Resolver {
            files,
            graph,
            by_path: BTreeMap::new(),
            attach: BTreeMap::new(),
            own: BTreeMap::new(),
            calls: BTreeMap::new(),
            callables: BTreeMap::new(),
        };
        for (i, file) in files.iter().enumerate() {
            r.by_path.insert(&file.file_path, i);
            for c in &file.callables {
                let c = &c.metadata;
                r.callables
                    .entry(bare(&c.name))
                    .or_default()
                    .push((i, &c.parameters));
            }
            for c in &file.call_statements {
                let c = &c.metadata;
                let callee = bare(c.function_name.rsplit('.').next().unwrap_or_default());
                r.calls.entry(callee).or_default().push(Site {
                    file: i,
                    scope: c
                        .enclosing_function_name
                        .as_deref()
                        .map(bare)
                        .unwrap_or_default()
                        .to_string(),
                    args: &c.arguments,
                });
            }
        }
        for (i, file_facts) in facts {
            for fact in file_facts {
                match fact {
                    RouteFact::Attach {
                        scope,
                        parent,
                        child,
                        prefix,
                    } => r
                        .attach
                        .entry((*i, scope_key(scope), child.clone()))
                        .or_default()
                        .push((parent.as_str(), prefix.as_str())),
                    RouteFact::Own {
                        scope,
                        router,
                        prefix,
                    } => r
                        .own
                        .entry((*i, scope_key(scope), router.clone()))
                        .or_default()
                        .push(prefix.as_str()),
                }
            }
        }
        r
    }

    /// Facts for `var` in `scope`, falling back to module level; returns the scope used.
    fn scoped<'m, 's, T>(
        map: &'m BTreeMap<VarKey, Vec<T>>,
        file: usize,
        scope: &'s str,
        var: &str,
    ) -> (&'s str, &'m [T]) {
        [scope, ""]
            .into_iter()
            .find_map(|s| {
                Some((
                    s,
                    map.get(&(file, s.to_string(), var.to_string()))?.as_slice(),
                ))
            })
            .unwrap_or_default()
    }

    /// Files whose callable `name` a call in `file` reaches (D5).
    fn callee_files(&self, file: usize, name: &str) -> BTreeSet<usize> {
        let candidates: Vec<usize> = self
            .callables
            .get(name)
            .map(|cs| cs.iter().map(|(f, _)| *f).collect())
            .unwrap_or_default();
        let imported = self
            .graph
            .lookup(&self.files[file].file_path, name)
            .and_then(|r| self.by_path.get(r.source_file.as_str()))
            .filter(|f| candidates.contains(f));
        if let Some(f) = imported {
            return BTreeSet::from([*f]);
        }
        if candidates.contains(&file) {
            return BTreeSet::from([file]);
        }
        let shared = |f: &usize| {
            let a = self.files[file].file_path.split('/');
            a.zip(self.files[*f].file_path.split('/'))
                .take_while(|(x, y)| x == y)
                .count()
        };
        let best = candidates.iter().map(shared).max().unwrap_or_default();
        candidates
            .into_iter()
            .filter(|f| shared(f) == best)
            .collect()
    }

    /// All prefixes under which router `var` in `scope` of `file` is served (D4).
    fn prefixes(&self, file: usize, scope: &str, var: &str, path: &mut Vec<VarKey>) -> Prefixes {
        let key = (file, scope.to_string(), var.to_string());
        if path.len() >= MAX_DEPTH || path.contains(&key) {
            return Prefixes::new();
        }
        path.push(key);
        let out = self
            .param_prefixes(file, scope, var, path)
            .unwrap_or_else(|| {
                let expr = |prefix: &str| PrefixExpr {
                    file_path: self.files[file].file_path.clone(),
                    expr: prefix.to_string(),
                };
                let mut outer = Prefixes::new();
                let (fact_scope, attach) = Self::scoped(&self.attach, file, scope, var);
                for (parent, prefix) in attach {
                    let chains = self.prefixes(file, fact_scope, parent, path);
                    merge(
                        &mut outer,
                        chains.into_iter().map(|mut chain| {
                            chain.push(expr(prefix));
                            chain
                        }),
                    );
                }
                if attach.is_empty() {
                    outer.insert(vec![]);
                }
                let (_, own) = Self::scoped(&self.own, file, scope, var);
                if own.is_empty() {
                    return outer;
                }
                let mut out = Prefixes::new();
                for prefix in own {
                    merge(
                        &mut out,
                        outer.iter().map(|chain| {
                            let mut chain = chain.clone();
                            chain.push(expr(prefix));
                            chain
                        }),
                    );
                }
                out
            });
        path.pop();
        out
    }

    /// `var` as parameter of callable `scope`: union over its callers' arguments.
    /// `None` when `var` is not a parameter.
    fn param_prefixes(
        &self,
        file: usize,
        scope: &str,
        var: &str,
        path: &mut Vec<VarKey>,
    ) -> Option<Prefixes> {
        let index = self
            .callables
            .get(scope)?
            .iter()
            .filter(|(f, _)| *f == file)
            .find_map(|(_, params)| params.iter().position(|p| p.name == var))?;
        let mut out = Prefixes::new();
        let mut callers = 0;
        for site in self.calls.get(scope).into_iter().flatten() {
            if !self.callee_files(site.file, scope).contains(&file) {
                continue;
            }
            callers += 1;
            let keyword = site.args.iter().find(|a| a.assigned_variable.trim() == var);
            let positional = site
                .args
                .iter()
                .filter(|a| a.assigned_variable.trim().is_empty())
                .nth(index);
            if let Some(arg) = keyword.or(positional) {
                merge(
                    &mut out,
                    self.prefixes(site.file, &site.scope, arg.value.trim(), path),
                );
            }
        }
        if callers == 0 {
            out.insert(vec![]);
        }
        Some(out)
    }
}

/// Resolver over adapter-produced facts; `facts` holds `(file index, facts)`.
fn resolve_with_facts(
    files: &mut [TypedFileRecord],
    facts: &[(usize, Vec<RouteFact>)],
    graph: &ImportGraph,
) {
    if facts.is_empty() {
        return;
    }
    let chains: Vec<Vec<Vec<Vec<PrefixExpr>>>> = {
        let r = Resolver::new(files, facts, graph);
        files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                file.endpoints
                    .iter()
                    .map(|e| {
                        let Some(router) = &e.router_variable else {
                            return vec![];
                        };
                        let scope = e
                            .enclosing_function
                            .as_deref()
                            .map(bare)
                            .unwrap_or_default();
                        let chains = r.prefixes(i, scope, router, &mut vec![]);
                        if chains.len() >= MAX_CHAINS {
                            warn!(
                                "route prefixes of {} in {} may be truncated at {MAX_CHAINS} chains",
                                e.uri, file.file_path
                            );
                        }
                        chains.into_iter().collect()
                    })
                    .collect()
            })
            .collect()
    };
    for (file, file_chains) in files.iter_mut().zip(chains) {
        for (endpoint, endpoint_chains) in file.endpoints.iter_mut().zip(file_chains) {
            endpoint.route_prefixes = endpoint_chains;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use models::{
        Argument, CallStatement, Callable, Endpoint, Namespace, Parameter, ParsedCallStatement,
        ParsedCallable, RouteFact, Scope,
        ir::{
            ast::CallableAst,
            language::Language,
            project::{ImportGraph, ImportKind, ResolvedImport, TypedFileRecord},
        },
        source_code::SourceSpan,
    };

    use super::{MAX_CHAINS, bare, resolve_with_facts};

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

    fn ep(uri: &str, router: Option<&str>, enclosing: Option<&str>) -> Endpoint {
        Endpoint {
            uri: uri.to_string(),
            router_variable: router.map(str::to_string),
            enclosing_function: enclosing.map(str::to_string),
            ..Default::default()
        }
    }

    /// Callable named like the Python extractor does: `name(params)`.
    fn callable(name: &str, params: &[&str]) -> ParsedCallable {
        ParsedCallable {
            metadata: Callable {
                name: format!("{name}({})", params.join(", ")),
                signature: String::new(),
                namespace: Namespace::default(),
                parameters: params
                    .iter()
                    .map(|p| Parameter::new(p.to_string(), None, None))
                    .collect(),
                return_type: None,
                is_async: false,
                is_constructor: false,
                hash: String::new(),
                file_path: String::new(),
            },
            ast: CallableAst {
                statements: vec![],
                nested: vec![],
            },
        }
    }

    /// `args`: `(keyword or "", value)`; `enclosing`: full signature text.
    fn call(callee: &str, args: &[(&str, &str)], enclosing: Option<&str>) -> ParsedCallStatement {
        ParsedCallStatement {
            metadata: CallStatement {
                function_name: callee.to_string(),
                arguments: args
                    .iter()
                    .map(|(k, v)| Argument {
                        assigned_variable: k.to_string(),
                        value: v.to_string(),
                        datatype: None,
                    })
                    .collect(),
                enclosing_function_name: enclosing.map(str::to_string),
                enclosing_class_name: None,
                enclosing_function_hash: None,
                is_self_invoke: false,
                is_super_invoke: false,
                invoked_on: None,
                source_span: SourceSpan::default(),
                is_decorator: false,
            },
            call_span: SourceSpan::default(),
            function_span: None,
        }
    }

    fn attach(parent: &str, child: &str, prefix: &str) -> RouteFact {
        RouteFact::Attach {
            scope: Scope::Global,
            parent: parent.to_string(),
            child: child.to_string(),
            prefix: prefix.to_string(),
        }
    }

    fn own(scope: Scope, router: &str, prefix: &str) -> RouteFact {
        RouteFact::Own {
            scope,
            router: router.to_string(),
            prefix: prefix.to_string(),
        }
    }

    fn graph(entries: &[(&str, &str, &str)]) -> ImportGraph {
        ImportGraph {
            resolved_imports: entries
                .iter()
                .map(|(importer, word, source)| {
                    (
                        (importer.to_string(), word.to_string()),
                        ResolvedImport {
                            source_file: source.to_string(),
                            fully_qualified_name: String::new(),
                            kind: ImportKind::Callable,
                        },
                    )
                })
                .collect(),
        }
    }

    /// `(endpoint file, uri, chains)`, chain parts rendered as `file: expr`.
    type Chains = Vec<(String, String, Vec<Vec<String>>)>;

    fn run(
        mut files: Vec<TypedFileRecord>,
        facts: Vec<(usize, Vec<RouteFact>)>,
        g: &ImportGraph,
    ) -> Chains {
        resolve_with_facts(&mut files, &facts, g);
        let mut out: Chains = files
            .iter()
            .flat_map(|f| {
                f.endpoints.iter().map(|e| {
                    let chains = e
                        .route_prefixes
                        .iter()
                        .map(|chain| {
                            chain
                                .iter()
                                .map(|p| format!("{}: {}", p.file_path, p.expr))
                                .collect()
                        })
                        .collect();
                    (f.file_path.clone(), e.uri.clone(), chains)
                })
            })
            .collect();
        out.sort();
        out
    }

    fn expected(items: &[(&str, &str, &[&[&str]])]) -> Chains {
        items
            .iter()
            .map(|(file, uri, chains)| {
                let chains = chains
                    .iter()
                    .map(|c| c.iter().map(|p| p.to_string()).collect())
                    .collect();
                (file.to_string(), uri.to_string(), chains)
            })
            .collect()
    }

    /// `app.py` mounts `app_v1` at `/v1` and passes it to `helper(app)` in `routes.py`.
    fn mounted_helper(
        dir: &str,
        prefix: &str,
    ) -> (Vec<TypedFileRecord>, Vec<(usize, Vec<RouteFact>)>) {
        let mut app = rec(&format!("{dir}/app.py"));
        app.call_statements
            .push(call("add_routes_v1", &[("", "app_v1")], None));
        let mut routes = rec(&format!("{dir}/api/routes.py"));
        routes.callables.push(callable("add_routes_v1", &["app"]));
        routes
            .endpoints
            .push(ep("/x", Some("app"), Some("add_routes_v1")));
        (
            vec![app, routes],
            vec![(0, vec![attach("app", "app_v1", &format!("\"{prefix}\""))])],
        )
    }

    #[test]
    fn bare_go_signatures() {
        assert_eq!(
            bare("func (s *Server) routes(r chi.Router) http.Handler"),
            "routes"
        );
        assert_eq!(bare("func main()"), "main");
        assert_eq!(bare("func Map[T any](x T)"), "Map");
        assert_eq!(bare("create_router() -> Any"), "create_router");
    }

    #[test]
    fn go_group_param_across_files() {
        let mut main = rec("svc/main.go");
        main.call_statements
            .push(call("registerRoutes", &[("", "api")], Some("func main()")));
        let mut routes = rec("svc/routes.go");
        routes.callables.push(callable("registerRoutes", &["g"]));
        routes
            .endpoints
            .push(ep("/health", Some("g"), Some("registerRoutes")));
        let fact = RouteFact::Attach {
            scope: Scope::Function("func main()".to_string()),
            parent: "r".to_string(),
            child: "api".to_string(),
            prefix: "\"/api\"".to_string(),
        };
        let out = run(vec![main, routes], vec![(0, vec![fact])], &graph(&[]));
        assert_eq!(
            out,
            expected(&[("svc/routes.go", "/health", &[&["svc/main.go: \"/api\""]])])
        );
    }

    #[test]
    fn attach_prefix() {
        let mut f = rec("svc/app.py");
        f.endpoints.push(ep("/x", Some("sub"), None));
        let out = run(
            vec![f],
            vec![(0, vec![attach("app", "sub", "\"/v3\"")])],
            &graph(&[]),
        );
        assert_eq!(
            out,
            expected(&[("svc/app.py", "/x", &[&["svc/app.py: \"/v3\""]])])
        );
    }

    #[test]
    fn own_then_outer_prefix() {
        let mut f = rec("svc/app.py");
        f.endpoints.push(ep("/{id}", Some("r"), None));
        let facts = vec![(
            0,
            vec![
                own(Scope::Global, "r", "\"/items\""),
                attach("app", "r", "\"/api\""),
            ],
        )];
        let out = run(vec![f], facts, &graph(&[]));
        assert_eq!(
            out,
            expected(&[(
                "svc/app.py",
                "/{id}",
                &[&["svc/app.py: \"/api\"", "svc/app.py: \"/items\""]]
            )])
        );
    }

    #[test]
    fn expression_kept_as_written() {
        let mut f = rec("svc/routes.py");
        f.endpoints
            .push(ep("/consigns", Some("router"), Some("create_router")));
        let scope = Scope::Function("create_router() -> Any".to_string());
        let facts = vec![(0, vec![own(scope, "router", "BASE_PATH")])];
        let out = run(vec![f], facts, &graph(&[]));
        assert_eq!(
            out,
            expected(&[(
                "svc/routes.py",
                "/consigns",
                &[&["svc/routes.py: BASE_PATH"]]
            )])
        );
    }

    #[test]
    fn param_followed_across_two_calls() {
        let mut app = rec("svc/app.py");
        app.call_statements
            .push(call("add_routes_v1", &[("", "app_v1")], None));
        let mut init = rec("svc/api/v1/__init__.py");
        init.callables.push(callable("add_routes_v1", &["app"]));
        init.call_statements.push(call(
            "add_routes_clinical",
            &[("", "app")],
            Some("add_routes_v1(app) -> Any"),
        ));
        let mut clinical = rec("svc/api/v1/clinical.py");
        clinical
            .callables
            .push(callable("add_routes_clinical", &["app"]));
        clinical.endpoints.push(ep(
            "/cases/{case_id}",
            Some("app"),
            Some("add_routes_clinical"),
        ));
        let facts = vec![(0, vec![attach("app", "app_v1", "\"/v1\"")])];
        let out = run(vec![app, init, clinical], facts, &graph(&[]));
        assert_eq!(
            out,
            expected(&[(
                "svc/api/v1/clinical.py",
                "/cases/{case_id}",
                &[&["svc/app.py: \"/v1\""]]
            )])
        );
    }

    #[test]
    fn callee_via_import_graph_beats_name_match() {
        let mut app = rec("svc/app.py");
        app.call_statements
            .push(call("add_routes_v1", &[("", "app_v1")], None));
        let mut one = rec("svc/one.py");
        one.callables.push(callable("add_routes_v1", &["app"]));
        one.endpoints
            .push(ep("/x", Some("app"), Some("add_routes_v1")));
        let mut two = rec("svc/two.py");
        two.callables.push(callable("add_routes_v1", &["app"]));
        two.endpoints
            .push(ep("/x", Some("app"), Some("add_routes_v1")));
        let facts = vec![(0, vec![attach("app", "app_v1", "\"/v1\"")])];
        let g = graph(&[("svc/app.py", "add_routes_v1", "svc/one.py")]);
        let out = run(vec![app, one, two], facts, &g);
        assert_eq!(
            out,
            expected(&[
                ("svc/one.py", "/x", &[&["svc/app.py: \"/v1\""]]),
                ("svc/two.py", "/x", &[&[]]),
            ])
        );
    }

    #[test]
    fn same_helper_name_other_service_not_mixed() {
        let (mut files, mut facts) = mounted_helper("a", "/v1");
        let (b_files, b_facts) = mounted_helper("b", "/v9");
        facts.extend(b_facts.into_iter().map(|(i, f)| (i + files.len(), f)));
        files.extend(b_files);
        let out = run(files, facts, &graph(&[]));
        assert_eq!(
            out,
            expected(&[
                ("a/api/routes.py", "/x", &[&["a/app.py: \"/v1\""]]),
                ("b/api/routes.py", "/x", &[&["b/app.py: \"/v9\""]]),
            ])
        );
    }

    fn reused_helper() -> (Vec<TypedFileRecord>, Vec<(usize, Vec<RouteFact>)>) {
        let mut app = rec("svc/app.py");
        app.call_statements
            .push(call("routes.add_routes", &[("", "app_v1")], None));
        app.call_statements
            .push(call("add_routes", &[("app", "app_v3")], None));
        let mut routes = rec("svc/routes.py");
        routes.callables.push(callable("add_routes", &["app"]));
        routes
            .endpoints
            .push(ep("/alive", Some("app"), Some("add_routes")));
        let facts = vec![(
            0,
            vec![
                attach("app", "app_v1", "\"/v1\""),
                attach("app", "app_v3", "\"/v3\""),
            ],
        )];
        (vec![app, routes], facts)
    }

    /// One endpoint, unchanged URI, two chains: pass 3 does the cloning.
    #[test]
    fn helper_reused_by_two_mounts_records_two_chains() {
        let (files, facts) = reused_helper();
        let out = run(files, facts, &graph(&[]));
        assert_eq!(
            out,
            expected(&[(
                "svc/routes.py",
                "/alive",
                &[&["svc/app.py: \"/v1\""], &["svc/app.py: \"/v3\""]]
            )])
        );
    }

    #[test]
    fn unknown_expression_recorded_verbatim() {
        let mut f = rec("svc/app.py");
        f.endpoints.push(ep("/x", Some("r"), None));
        let facts = vec![(
            0,
            vec![
                own(Scope::Global, "r", "settings.base"),
                attach("app", "r", "\"/api\""),
            ],
        )];
        let out = run(vec![f], facts, &graph(&[]));
        assert_eq!(
            out,
            expected(&[(
                "svc/app.py",
                "/x",
                &[&["svc/app.py: \"/api\"", "svc/app.py: settings.base"]]
            )])
        );
    }

    #[test]
    fn recursive_helper_terminates() {
        let mut app = rec("svc/app.py");
        app.call_statements.push(call("h", &[("", "app_v1")], None));
        let mut h = rec("svc/h.py");
        h.callables.push(callable("h", &["app"]));
        h.call_statements
            .push(call("h", &[("", "app")], Some("h(app) -> Any")));
        h.endpoints.push(ep("/x", Some("app"), Some("h")));
        let facts = vec![(0, vec![attach("app", "app_v1", "\"/v1\"")])];
        let out = run(vec![app, h], facts, &graph(&[]));
        assert_eq!(
            out,
            expected(&[("svc/h.py", "/x", &[&["svc/app.py: \"/v1\""]])])
        );
    }

    #[test]
    fn no_facts_no_change() {
        let mut f = rec("svc/App.java");
        f.endpoints.push(ep("/x", None, None));
        let out = run(
            vec![f],
            vec![(0, vec![attach("app", "sub", "\"/v3\"")])],
            &graph(&[]),
        );
        assert_eq!(out, expected(&[("svc/App.java", "/x", &[])]));
    }

    #[test]
    fn output_independent_of_file_order() {
        let case = || {
            let (mut files, mut facts) = reused_helper();
            let (more, more_facts) = mounted_helper("b", "/v9");
            facts.extend(more_facts.into_iter().map(|(i, f)| (i + files.len(), f)));
            files.extend(more);
            (files, facts)
        };
        let (files, facts) = case();
        let forward = run(files, facts, &graph(&[]));

        let (mut files, facts) = case();
        let last = files.len() - 1;
        files.reverse();
        let facts = facts.into_iter().map(|(i, f)| (last - i, f)).collect();
        let backward = run(files, facts, &graph(&[]));
        assert_eq!(forward, backward);
        // Chains only: pass 2 never clones.
        assert_eq!(forward.len(), 2);
    }

    /// 7 attach levels, 4 non-literal mounts each: 4^7 chains uncapped.
    #[test]
    fn chains_capped_deterministically() {
        let facts: Vec<RouteFact> = (1..=7)
            .flat_map(|level| {
                (0..4).map(move |width| {
                    attach(
                        &format!("r{}", level + 1),
                        &format!("r{level}"),
                        &format!("settings.a{level}_{width}"),
                    )
                })
            })
            .collect();
        let case = |facts: Vec<RouteFact>| {
            let mut f = rec("svc/app.py");
            f.endpoints.push(ep("/x", Some("r1"), None));
            run(vec![f], vec![(0, facts)], &graph(&[]))
        };
        let forward = case(facts.clone());
        assert_eq!(forward[0].2.len(), MAX_CHAINS);
        assert_eq!(forward, case(facts.into_iter().rev().collect()));
    }
}
