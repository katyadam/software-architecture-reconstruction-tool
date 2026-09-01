use crate::extraction::endpoints::{
    EndpointStrategy, PythonEndpointStrategy,
    identification::{collect_decorator_matches, decorator_query, strategies},
};
use crate::extraction::extractor::{ExtractParams, Extractor};
use models::Endpoint;
use tree_sitter::Query;

/// Backwards-compatible facade for callers that used the original extractor.
pub struct EndpointsExtractor;

impl Extractor for EndpointsExtractor {
    type Item<'a> = Endpoint;

    fn query(&self) -> &'static Query {
        PythonEndpointStrategy.query()
    }

    fn extract<'a>(&self, params: ExtractParams<'a>) -> Vec<Self::Item<'a>> {
        Extractor::extract(&PythonEndpointStrategy, params)
    }
}

impl Extractor for PythonEndpointStrategy {
    type Item<'a> = Endpoint;

    fn query(&self) -> &'static Query {
        decorator_query()
    }

    fn extract<'a>(&self, params: ExtractParams<'a>) -> Vec<Self::Item<'a>> {
        let decorator_matches = collect_decorator_matches(params);
        let mut endpoints = Vec::new();

        for strategy in strategies() {
            endpoints.extend(strategy.identify_endpoints(params, &decorator_matches));
        }

        endpoints
    }
}

impl EndpointStrategy for PythonEndpointStrategy {
    fn extract<'a>(&self, params: ExtractParams<'a>) -> Vec<Endpoint> {
        <Self as Extractor>::extract(self, params)
    }
}
