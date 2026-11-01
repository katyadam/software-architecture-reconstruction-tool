use models::{RouteFact, ir::project::TypedFileRecord};

use super::idiom::{AttachCall, OwnCtor, facts_from_idioms};

/// FastAPI / Starlette: `mount`, `include_router`, `APIRouter(prefix=)`.
const ATTACH: [AttachCall; 2] = [
    AttachCall {
        method: "mount",
        child: ("app", Some(1)),
        prefix: ("path", Some(0)),
    },
    AttachCall {
        method: "include_router",
        child: ("router", Some(0)),
        prefix: ("prefix", None),
    },
];
const OWN: [OwnCtor; 1] = [OwnCtor {
    ctor: "APIRouter",
    prefix_kw: "prefix",
}];

pub(super) fn facts(file: &TypedFileRecord) -> Vec<RouteFact> {
    facts_from_idioms(file, &ATTACH, &OWN)
}
