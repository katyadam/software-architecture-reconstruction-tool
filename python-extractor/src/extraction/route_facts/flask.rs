use models::{RouteFact, ir::project::TypedFileRecord};

use super::idiom::{AttachCall, OwnCtor, facts_from_idioms};

/// Flask: `register_blueprint(bp, url_prefix=)`, `Blueprint(..., url_prefix=)`.
const ATTACH: [AttachCall; 1] = [AttachCall {
    method: "register_blueprint",
    child: ("blueprint", Some(0)),
    prefix: ("url_prefix", None),
}];
const OWN: [OwnCtor; 1] = [OwnCtor {
    ctor: "Blueprint",
    prefix_kw: "url_prefix",
}];

pub(super) fn facts(file: &TypedFileRecord) -> Vec<RouteFact> {
    facts_from_idioms(file, &ATTACH, &OWN)
}
