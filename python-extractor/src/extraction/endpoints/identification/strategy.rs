use crate::extraction::endpoints::identification::DecoratorEndpointMatch;
use crate::extraction::extractor::ExtractParams;
use models::Endpoint;

pub(crate) trait IdentificationStrategy {
    fn identify_endpoints<'a>(
        &self,
        params: ExtractParams<'a>,
        decorator_matches: &[DecoratorEndpointMatch],
    ) -> Vec<Endpoint>;
}
