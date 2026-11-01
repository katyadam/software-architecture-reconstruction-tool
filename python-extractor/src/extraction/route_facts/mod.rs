mod fastapi;
mod flask;
mod idiom;

use models::{RouteFact, ir::project::TypedFileRecord};

/// Per-framework adapters; a new framework is one file plus one entry here.
const FRAMEWORKS: [fn(&TypedFileRecord) -> Vec<RouteFact>; 2] = [fastapi::facts, flask::facts];

/// Router composition facts from every known Python framework.
pub fn route_facts(file: &TypedFileRecord) -> Vec<RouteFact> {
    FRAMEWORKS.iter().flat_map(|f| f(file)).collect()
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

    #[test]
    fn flask_register_blueprint_and_blueprint_prefix() {
        let code = "bp = Blueprint(\"orders\", __name__, url_prefix=\"/orders\")\n\
                    app.register_blueprint(bp, url_prefix=\"/api\")\n\
                    app.register_blueprint(bp)\n";
        assert_eq!(
            facts(code),
            vec![
                attach("app", "bp", "\"/api\""),
                attach("app", "bp", "\"\""),
                RouteFact::Own {
                    scope: Scope::Global,
                    router: "bp".to_string(),
                    prefix: "\"/orders\"".to_string(),
                },
            ]
        );
    }
}
