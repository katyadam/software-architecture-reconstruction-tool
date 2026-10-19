//! Provider-neutral call-graph analysis and source-callable resolution.

mod registry;
mod resolver;
pub mod wala;
pub use registry::{CallGraphProvider, ProviderRegistry};
pub use resolver::{
    JavaCallGraphResolution, resolve_edges, resolve_edges_with_java_tests,
    resolve_reachability_with_java_tests,
};
