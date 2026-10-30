use crate::Scope;

/// Language-neutral fact about how a router contributes a URL prefix.
/// `prefix` is the raw expression text (quoted literal or identifier).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteFact {
    /// `child` is served under `parent` + `prefix`.
    Attach {
        scope: Scope,
        parent: String,
        child: String,
        prefix: String,
    },
    /// `router` adds `prefix` to its own routes.
    Own {
        scope: Scope,
        router: String,
        prefix: String,
    },
}
