use crate::extraction::endpoints::identification::{
    DecoratorEndpointMatch, IdentificationStrategy,
};
use crate::extraction::extractor::ExtractParams;
use models::{Endpoint, HttpMethod};

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct MethodDecoratorIdentificationStrategy;

impl IdentificationStrategy for MethodDecoratorIdentificationStrategy {
    fn identify_endpoints<'a>(
        &self,
        params: ExtractParams<'a>,
        decorator_matches: &[DecoratorEndpointMatch],
    ) -> Vec<Endpoint> {
        decorator_matches
            .iter()
            .filter_map(|candidate| {
                let method = candidate.http_method.parse::<HttpMethod>().ok()?;
                Some(Endpoint {
                    function_name: candidate.function_name.clone(),
                    function_hash: candidate.function_hash.clone(),
                    http_method: method,
                    parameters: candidate.parameters.clone(),
                    uri: candidate.uri.clone(),
                    file_path: params.file_name.unwrap_or_default().to_string(),
                    router_variable: candidate.router_variable.clone(),
                })
            })
            .collect()
    }
}
