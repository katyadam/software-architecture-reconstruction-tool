use crate::resolver::facts::FactBundle;

/// The category of resolution question sent to Sage.
pub enum QueryKind {
    ResolveEnvVar { var_name: String },
    ResolveBuilder { chain: String },
    ResolveLookup { lookup_key: String },
    ResolveFrameworkRoute { route_pattern: String },
    ResolveReflective { target: String },
    ClassifyHttpCall { call_expr: String },
}

/// A question to Sage, pairing context facts with a resolution kind.
pub struct SageQuery {
    pub bundle: FactBundle,
    pub kind: QueryKind,
}
