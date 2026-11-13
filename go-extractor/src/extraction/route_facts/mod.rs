use models::{AssignmentKey, CallStatement, RouteFact, Scope, ir::project::TypedFileRecord};

use super::endpoint_frameworks::{closure_routers, scoped_router};

/// `child := parent.<method>(prefix, ...)<suffix>`: Gin / Echo / Fiber groups, gorilla subrouters.
const ASSIGNED: [(&str, &str); 2] = [("Group", ""), ("PathPrefix", ".Subrouter()")];

/// Router composition facts for Go: groups, gorilla subrouters, mounts, closure routers.
pub fn route_facts(file: &TypedFileRecord) -> Vec<RouteFact> {
    let assigned = file.assignments.iter().filter_map(|(key, a)| {
        let value = a.value.trim();
        ASSIGNED.iter().find_map(|(method, suffix)| {
            let call = value.strip_suffix(suffix)?.strip_suffix(')')?;
            let (parent, args) = call.split_once(&format!(".{method}("))?;
            // ponytail: naive `,` split; a prefix literal holding `,` is dropped, split at top level if seen.
            let prefix = args.split(',').next()?;
            // Package-level var assigned in a function (`api = r.Group(..)` in `init`): fact is global.
            let global = AssignmentKey {
                scope: Scope::Global,
                variable_name: key.variable_name.clone(),
            };
            let scope = if file.assignments.contains_key(&global) {
                &Scope::Global
            } else {
                &key.scope
            };
            attach(file, scope, parent, &key.variable_name, prefix)
        })
    });
    let closures = closure_routers(&file.call_statements, &file.imports);
    let closed = closures.iter().filter_map(|closure| {
        let parent = scoped_router(
            &closures,
            closure.call.call_span.start_byte,
            closure.receiver,
        );
        attach(
            file,
            &call_scope(&closure.call.metadata),
            &parent,
            &closure.variable(),
            closure.prefix,
        )
    });
    let mounted = file.call_statements.iter().filter_map(|call| {
        let c = &call.metadata;
        let (parent, method) = c.function_name.rsplit_once('.')?;
        let [prefix, child] = call.raw_arguments.as_slice() else {
            return None;
        };
        let child = child.trim();
        // `parent.Mount(prefix, child)`: chi / Fiber mounts.
        if method != "Mount" || !is_identifier(child) {
            return None;
        }
        let parent = scoped_router(&closures, call.call_span.start_byte, parent.trim());
        attach(file, &call_scope(c), &parent, child, prefix)
    });
    assigned.chain(closed).chain(mounted).collect()
}

fn call_scope(call: &CallStatement) -> Scope {
    call.enclosing_function_name
        .clone()
        .map_or(Scope::Global, Scope::Function)
}

fn attach(
    file: &TypedFileRecord,
    scope: &Scope,
    parent: &str,
    child: &str,
    prefix: &str,
) -> Option<RouteFact> {
    Some(RouteFact::Attach {
        scope: scope.clone(),
        parent: parent.trim().to_string(),
        child: child.to_string(),
        prefix: prefix_text(file, scope, prefix.trim())?,
    })
}

/// Prefix as a `"..."` literal; identifiers resolve to same-file constants (D2), else stay as-is.
fn prefix_text(file: &TypedFileRecord, scope: &Scope, expr: &str) -> Option<String> {
    // Single literal only: `"/a").Group("/b"` is a chain, not a prefix.
    let literal = ['"', '`'].into_iter().find_map(|q| {
        let inner = expr.strip_prefix(q)?.strip_suffix(q)?;
        (!inner.contains(['"', '`'])).then(|| format!("\"{inner}\""))
    });
    if literal.is_some() {
        return literal;
    }
    if !is_identifier(expr) {
        return None;
    }
    // Go stores evaluated values unquoted; only path-like ones count as string constants.
    let constant = [scope, &Scope::Global]
        .into_iter()
        .find_map(|scope| {
            file.assignments.get(&AssignmentKey {
                scope: scope.clone(),
                variable_name: expr.to_string(),
            })
        })
        .map(|a| a.value.trim())
        .filter(|value| value.starts_with('/'));
    Some(constant.map_or_else(|| expr.to_string(), |value| format!("\"{value}\"")))
}

fn is_identifier(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
        && text.chars().all(|c| c.is_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use models::{RouteFact, Scope};

    use super::route_facts;
    use crate::extraction::extract_syntactic;

    fn facts(code: &str) -> Vec<RouteFact> {
        let record: models::ir::project::TypedFileRecord = extract_syntactic(code, "svc/main.go")
            .expect("Go parses")
            .into();
        route_facts(&record)
    }

    fn attach(function: &str, parent: &str, child: &str, prefix: &str) -> RouteFact {
        RouteFact::Attach {
            scope: Scope::Function(function.to_string()),
            parent: parent.to_string(),
            child: child.to_string(),
            prefix: prefix.to_string(),
        }
    }

    #[test]
    fn group_assignment() {
        let code = "package main\n\nfunc startRest() {\n\trouter := gin.Default()\n\tapi := router.Group(\"/api\")\n\tapi.GET(\"/health\", health)\n}\n";
        assert_eq!(
            facts(code),
            vec![attach("func startRest()", "router", "api", "\"/api\"")]
        );
    }

    #[test]
    fn group_with_middleware() {
        let code =
            "package main\n\nfunc f() {\n\tr := gin.New()\n\tv1 := r.Group(\"/v1\", auth)\n}\n";
        assert_eq!(facts(code), vec![attach("func f()", "r", "v1", "\"/v1\"")]);
    }

    #[test]
    fn gorilla_subrouter() {
        let code = "package main\n\nfunc setup() {\n\tr := mux.NewRouter()\n\ts := r.PathPrefix(\"/orders\").Subrouter()\n}\n";
        assert_eq!(
            facts(code),
            vec![attach("func setup()", "r", "s", "\"/orders\"")]
        );
    }

    #[test]
    fn chi_mount_identifier() {
        let code = "package main\n\nfunc routes() {\n\tr := chi.NewRouter()\n\tadmin := chi.NewRouter()\n\tr.Mount(\"/admin\", admin)\n}\n";
        assert_eq!(
            facts(code),
            vec![attach("func routes()", "r", "admin", "\"/admin\"")]
        );
    }

    #[test]
    fn mount_call_result_ignored() {
        let code = "package main\n\nfunc routes() {\n\tr := chi.NewRouter()\n\tr.Mount(\"/admin\", adminRouter())\n}\n";
        assert_eq!(facts(code), vec![]);
    }

    #[test]
    fn chi_group_closure_attaches_empty_prefix() {
        let code = "package main\n\nimport \"github.com/go-chi/chi/v5\"\n\nfunc routes() {\n\tr := chi.NewRouter()\n\tr.Group(func(r chi.Router) {})\n}\n";
        let child = format!("r@{}", code.rfind(')').expect("call in code") + 1);
        assert_eq!(
            facts(code),
            vec![attach("func routes()", "r", &child, "\"\"")]
        );
    }

    #[test]
    fn chi_route_closure_attaches_prefix() {
        let code = "package main\n\nimport \"github.com/go-chi/chi/v5\"\n\nfunc routes() {\n\tr := chi.NewRouter()\n\tr.Route(\"/p\", func(sub chi.Router) {})\n}\n";
        let child = format!("sub@{}", code.rfind(')').expect("call in code") + 1);
        assert_eq!(
            facts(code),
            vec![attach("func routes()", "r", &child, "\"/p\"")]
        );
    }

    #[test]
    fn chi_route_closure_unresolved_prefix_has_no_fact() {
        let code = "package main\n\nimport \"github.com/go-chi/chi/v5\"\n\nfunc routes() {\n\tr := chi.NewRouter()\n\tr.Route(cfg.Base, func(r chi.Router) {})\n}\n";
        assert_eq!(facts(code), vec![]);
    }

    #[test]
    fn mount_in_closure_attaches_to_closure_router() {
        let code = "package main\n\nimport \"github.com/go-chi/chi/v5\"\n\nfunc routes() {\n\tr := chi.NewRouter()\n\tadmin := chi.NewRouter()\n\tr.Route(\"/p\", func(r chi.Router) { r.Mount(\"/admin\", admin) })\n}\n";
        let closure = format!("r@{}", code.rfind(')').expect("call in code") + 1);
        let found = facts(code);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found.contains(&attach("func routes()", "r", &closure, "\"/p\"")));
        assert!(found.contains(&attach("func routes()", &closure, "admin", "\"/admin\"")));
    }

    #[test]
    fn package_level_group_var_is_global() {
        let code = "package main\n\nvar api *gin.RouterGroup\n\nfunc init() {\n\tr := gin.Default()\n\tapi = r.Group(\"/api\")\n}\n";
        assert_eq!(
            facts(code),
            vec![RouteFact::Attach {
                scope: Scope::Global,
                parent: "r".to_string(),
                child: "api".to_string(),
                prefix: "\"/api\"".to_string(),
            }]
        );
    }

    #[test]
    fn group_prefix_from_const() {
        let code = "package main\n\nconst apiBase = \"/api\"\n\nfunc f() {\n\te := echo.New()\n\tg := e.Group(apiBase)\n}\n";
        assert_eq!(facts(code), vec![attach("func f()", "e", "g", "\"/api\"")]);
    }
}
