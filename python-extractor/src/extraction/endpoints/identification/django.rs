use crate::extraction::calls::extractor::CallsExtractor;
use crate::extraction::endpoints::identification::{
    DecoratorEndpointMatch, IdentificationStrategy, decorator::clean_python_string,
};
use crate::extraction::extractor::{ExtractParams, Extractor};
use models::{Endpoint, HttpMethod};

use super::flask::python_http_method_list;

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct DjangoApiViewIdentificationStrategy;

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct DjangoUrlpatternsIdentificationStrategy;

impl IdentificationStrategy for DjangoApiViewIdentificationStrategy {
    fn identify_endpoints<'a>(
        &self,
        params: ExtractParams<'a>,
        decorator_matches: &[DecoratorEndpointMatch],
    ) -> Vec<Endpoint> {
        decorator_matches
            .iter()
            .filter(|candidate| candidate.http_method == "api_view")
            .flat_map(|candidate| {
                python_http_method_list(&candidate.decorator_args)
                    .into_iter()
                    .map(|method| Endpoint {
                        function_name: candidate.function_name.clone(),
                        function_hash: candidate.function_hash.clone(),
                        http_method: method,
                        parameters: candidate.parameters.clone(),
                        uri: String::new(),
                        file_path: params.file_name.unwrap_or_default().to_string(),
                        router_variable: None,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

impl IdentificationStrategy for DjangoUrlpatternsIdentificationStrategy {
    fn identify_endpoints<'a>(
        &self,
        params: ExtractParams<'a>,
        _decorator_matches: &[DecoratorEndpointMatch],
    ) -> Vec<Endpoint> {
        let calls = CallsExtractor.extract(params);
        calls
            .iter()
            .filter(|call| {
                matches!(
                    call.call_statement.function_name.as_str(),
                    "path" | "re_path"
                )
            })
            .filter_map(|call| django_urlpattern_endpoint(call.node, params))
            .collect()
    }
}

fn django_urlpattern_endpoint(
    call_node: tree_sitter::Node,
    params: ExtractParams<'_>,
) -> Option<Endpoint> {
    let call_text = &params.code[call_node.start_byte()..call_node.end_byte()];
    let open = call_text.find('(')?;
    let close = call_text.rfind(')')?;
    let arguments = statix::strings::split_at_top_level(
        &call_text[open + 1..close],
        &[','],
        &[('(', ')'), ('[', ']'), ('{', '}')],
    );
    let route = clean_python_string(arguments.first()?);
    let view = arguments.get(1)?.trim();
    if view.contains("include(") {
        return None;
    }

    Some(Endpoint {
        function_name: django_view_name(view),
        function_hash: String::new(),
        http_method: HttpMethod::GET,
        parameters: Vec::new(),
        uri: super::decorator::normalize_python_route(&route),
        file_path: params.file_name.unwrap_or_default().to_string(),
        router_variable: Some("urlpatterns".to_string()),
    })
}

fn django_view_name(view: &str) -> String {
    let cleaned = view.trim();
    let without_call = cleaned.split('(').next().unwrap_or(cleaned);
    without_call
        .rsplit('.')
        .find(|part| !part.is_empty() && *part != "as_view")
        .unwrap_or(without_call)
        .to_string()
}
