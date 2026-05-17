use models::assignments::VariableAddress;

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
///
/// `variables_map` is an **ordered** list of variable entries. Callers are
/// responsible for ranking and capping the list before constructing the
/// query
pub struct SageQuery {
    pub bundle: FactBundle,
    pub kind: QueryKind,
    pub variables_map: Vec<(VariableAddress, String)>,
}
