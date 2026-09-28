mod registry;
mod resolver;
pub mod wala;
pub use registry::{CallGraphProvider, ProviderRegistry};
pub use resolver::resolve_edges;
