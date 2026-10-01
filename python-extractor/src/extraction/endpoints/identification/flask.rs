use crate::extraction::endpoints::identification::{
    DecoratorEndpointMatch, IdentificationStrategy, decorator::clean_python_string,
};
use crate::extraction::extractor::ExtractParams;
use models::{Endpoint, HttpMethod};

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct FlaskRouteIdentificationStrategy;

impl IdentificationStrategy for FlaskRouteIdentificationStrategy {
    fn identify_endpoints<'a>(
        &self,
        params: ExtractParams<'a>,
        decorator_matches: &[DecoratorEndpointMatch],
    ) -> Vec<Endpoint> {
        decorator_matches
            .iter()
            .filter(|candidate| candidate.http_method == "route")
            .flat_map(|candidate| {
                let methods = flask_route_methods(&candidate.decorator);
                let methods = if methods.is_empty() {
                    vec![HttpMethod::GET]
                } else {
                    methods
                };

                methods.into_iter().map(|method| Endpoint {
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

fn flask_route_methods(decorator: &str) -> Vec<HttpMethod> {
    let Some(methods_start) = decorator.find("methods") else {
        return Vec::new();
    };
    let Some(list_start) = decorator[methods_start..]
        .find('[')
        .map(|idx| idx + methods_start)
    else {
        return Vec::new();
    };
    let Some(list_end) = decorator[list_start..]
        .find(']')
        .map(|idx| idx + list_start)
    else {
        return Vec::new();
    };

    python_http_method_list(&decorator[list_start..=list_end])
}

pub(super) fn python_http_method_list(raw: &str) -> Vec<HttpMethod> {
    let value = raw.trim();
    let list = value
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();

    statix::strings::split_at_top_level(list, &[','], &[('(', ')'), ('[', ']')])
        .into_iter()
        .filter_map(|method| clean_python_string(&method).parse().ok())
        .collect()
}
