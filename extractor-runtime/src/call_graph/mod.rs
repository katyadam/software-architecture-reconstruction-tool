//! Provider-neutral call-graph analysis and source-callable resolution.

mod adapter_process;
pub mod go_native;
pub mod java_adapter;
mod registry;
mod resolver;
pub mod wala;
pub use registry::{CallGraphProvider, ProviderRegistry};
pub use resolver::{
    JavaCallGraphResolution, resolve_edges, resolve_edges_with_java_tests,
    resolve_reachability_with_java_tests,
};
