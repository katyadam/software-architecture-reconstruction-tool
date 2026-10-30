use models::{RouteFact, Scope, ir::project::TypedFileRecord};

/// FastAPI / Starlette router composition facts: `mount`, `include_router`, `APIRouter(prefix=)`.
pub fn route_facts(file: &TypedFileRecord) -> Vec<RouteFact> {
    let calls = file.call_statements.iter().filter_map(|c| {
        let c = &c.metadata;
        let (parent, method) = c.function_name.rsplit_once('.')?;
        // `(keyword, position)` of the child and prefix arguments.
        let ((child_kw, child_pos), (prefix_kw, prefix_pos)) = match method {
            "mount" => (("app", 1), ("path", Some(0))),
            "include_router" => (("router", 0), ("prefix", None)),
            _ => return None,
        };
        let arg = |kw: &str, pos: Option<usize>| {
            let keyword = c
                .arguments
                .iter()
                .find(|a| a.assigned_variable.trim() == kw);
            let positional = || {
                c.arguments
                    .iter()
                    .filter(|a| a.assigned_variable.trim().is_empty())
                    .nth(pos?)
            };
            keyword
                .or_else(positional)
                .map(|a| a.value.trim().to_string())
        };
        Some(RouteFact::Attach {
            scope: c
                .enclosing_function_name
                .clone()
                .map_or(Scope::Global, Scope::Function),
            parent: parent.trim().to_string(),
            child: arg(child_kw, Some(child_pos))?,
            // No prefix -> empty literal, which the resolver evaluates to `""`.
            prefix: arg(prefix_kw, prefix_pos).unwrap_or_else(|| "\"\"".to_string()),
        })
    });
    // ponytail: naive `,` split of the constructor text; commas inside literals break it.
    let own = file.assignments.iter().filter_map(|(key, a)| {
        let args = a
            .value
            .trim()
            .strip_prefix("APIRouter(")?
            .strip_suffix(')')?;
        let prefix = args.split(',').find_map(|arg| {
            arg.trim()
                .strip_prefix("prefix")?
                .trim_start()
                .strip_prefix('=')
        })?;
        Some(RouteFact::Own {
            scope: key.scope.clone(),
            router: key.variable_name.clone(),
            prefix: prefix.trim().to_string(),
        })
    });
    calls.chain(own).collect()
}

#[cfg(test)]
mod tests {
    use models::{RouteFact, Scope};

    use super::route_facts;
    use crate::extraction::parse::extract_syntactic;

    fn facts(code: &str) -> Vec<RouteFact> {
        let record: models::ir::project::TypedFileRecord =
            extract_syntactic(code, "svc/app.py").unwrap().into();
        route_facts(&record)
    }

    fn attach(parent: &str, child: &str, prefix: &str) -> RouteFact {
        RouteFact::Attach {
            scope: Scope::Global,
            parent: parent.to_string(),
            child: child.to_string(),
            prefix: prefix.to_string(),
        }
    }

    #[test]
    fn mount_positional_and_keyword() {
        let code = "app.mount(\"/v1\", app_v1)\napp.mount(path=\"/v3/fhir\", app=app_v3)\n";
        assert_eq!(
            facts(code),
            vec![
                attach("app", "app_v1", "\"/v1\""),
                attach("app", "app_v3", "\"/v3/fhir\""),
            ]
        );
    }

    #[test]
    fn include_router_with_and_without_prefix() {
        let code = "app.include_router(r, prefix=\"/api\")\napp.include_router(r)\n";
        assert_eq!(
            facts(code),
            vec![attach("app", "r", "\"/api\""), attach("app", "r", "\"\"")]
        );
    }

    #[test]
    fn api_router_own_prefix() {
        let code =
            "def create_router(s):\n    router = APIRouter(prefix=BASE_PATH)\n    return router\n";
        let out = facts(code);
        let [
            RouteFact::Own {
                scope: Scope::Function(name),
                router,
                prefix,
            },
        ] = out.as_slice()
        else {
            panic!("expected one Own fact, got {out:?}");
        };
        assert!(name.starts_with("create_router("), "{name}");
        assert_eq!((router.as_str(), prefix.as_str()), ("router", "BASE_PATH"));
    }

    #[test]
    fn unrelated_calls_ignored() {
        let code = "client.mount_volume(x)\nrouter = APIRouter()\n";
        assert_eq!(facts(code), vec![]);
    }
}
