pub mod extractor;

use crate::extraction::extractor::ExtractParams;
use models::Endpoint;

/// Strategy used to discover HTTP endpoints in a source file.
///
/// The runtime only needs the language-neutral result. Framework-specific
/// syntax (FastAPI/Flask/Django) belongs in the Python implementation.
pub trait EndpointStrategy {
    fn extract<'a>(&self, params: ExtractParams<'a>) -> Vec<Endpoint>;
}

/// Python endpoint strategy. It understands decorator-based frameworks and
/// Django's `urlpatterns` calls.
#[derive(Debug, Default, Clone, Copy)]
pub struct PythonEndpointStrategy;
