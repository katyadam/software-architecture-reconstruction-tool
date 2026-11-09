//! Provider-neutral call-graph analysis and source-callable resolution.

mod adapter_process;
pub mod go_native;
pub mod java_adapter;
pub mod pyan3;
mod registry;
mod resolver;
pub mod wala;
pub mod spoon;
pub use registry::{CallGraphProvider, ProviderRegistry};
pub use resolver::{
    JavaCallGraphResolution, JavaSourceLayout, resolve_edges, resolve_edges_with_java_layout,
    resolve_edges_with_java_tests, resolve_reachability_with_java_layout,
    resolve_reachability_with_java_tests,
};
