use crate::extraction::common::extract_param_names;
use crate::extraction::extractor::{ExtractParams, Extractor};
use crate::extraction::queries::ENDPOINTS_QUERY;
use models::{Endpoint, HttpMethod};
use tree_sitter::StreamingIterator;
use tree_sitter::{Query, QueryCursor};

pub struct EndpointsExtractor;

impl Extractor<Endpoint> for EndpointsExtractor {
    fn extract(&self, params: ExtractParams) -> Vec<Endpoint> {
        let query = Query::new(&tree_sitter_python::LANGUAGE.into(), ENDPOINTS_QUERY).unwrap();

        let mut query_cursor = QueryCursor::new();
        let matches = query_cursor.matches(&query, params.tree.root_node(), params.code.as_bytes());
        let mut endpoints: Vec<Endpoint> = vec![];
        matches.for_each(|m| {
            let mut function_name = String::new();
            let mut http_method = String::new();
            let mut uri = String::new();
            let mut arguments = vec![];

            m.captures.into_iter().for_each(|capture| {
                let capture_text =
                    &params.code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();
                match query.capture_names()[capture.index as usize] {
                    "function.name" => function_name = value,
                    "http.method" => http_method = value,
                    "http.uri" => uri = value.trim_matches('"').to_string(),
                    "function.params" => {
                        let param_names = extract_param_names(capture.node, &params.code);
                        arguments.extend(param_names);
                    }
                    _ => {}
                }
            });

            endpoints.push(Endpoint {
                function_name: function_name,
                http_method: http_method.parse().unwrap_or(HttpMethod::GET),
                parameters: arguments,
                uri,
                service_name: params.service_name.unwrap_or_default().to_string(),
            });
        });
        endpoints
    }
}
