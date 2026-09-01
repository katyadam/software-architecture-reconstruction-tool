pub mod extractor;
mod identification;

use crate::extraction::extractor::ExtractParams;
use models::Endpoint;

/// Facade used to discover Python HTTP endpoints in a source file.
///
/// Framework-specific endpoint identification lives in internal strategies
/// rather than in the extractor runtime.
pub trait EndpointStrategy {
    fn extract<'a>(&self, params: ExtractParams<'a>) -> Vec<Endpoint>;
}

/// Python endpoint facade that delegates to framework-specific strategies.
#[derive(Debug, Default, Clone, Copy)]
pub struct PythonEndpointStrategy;
