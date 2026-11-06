//! Provider-neutral call-graph analysis and source-callable resolution.

mod registry;
mod resolver;
pub mod wala;
pub use registry::{CallGraphProvider, ProviderRegistry};
pub use resolver::resolve_edges;
