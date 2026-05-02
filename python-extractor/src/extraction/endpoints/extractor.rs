use std::sync::OnceLock;

use crate::extraction::callables::parser::parse_parameters;
use crate::extraction::extractor::{ExtractParams, Extractor};
use crate::extraction::queries::ENDPOINTS_QUERY;
use models::{Endpoint, HttpMethod};
use tree_sitter::StreamingIterator;
use tree_sitter::{Query, QueryCursor};

pub struct EndpointsExtractor;

impl Extractor for EndpointsExtractor {
    type Item<'a> = Endpoint;

    fn query(&self) -> &'static Query {
        static QUERY: OnceLock<Query> = OnceLock::new();

        QUERY.get_or_init(|| {
            Query::new(&tree_sitter_python::LANGUAGE.into(), ENDPOINTS_QUERY)
                .expect("Failed to compile Python Endpoints Query")
        })
    }

    fn extract<'a>(&self, params: ExtractParams<'a>) -> Vec<Self::Item<'a>> {
        let query = self.query();

        let mut query_cursor = QueryCursor::new();
        let matches = query_cursor.matches(query, params.tree.root_node(), params.code.as_bytes());
        let mut endpoints: Vec<Endpoint> = vec![];
        matches.for_each(|m| {
            let mut function_name = String::new();
            let mut http_method = String::new();
            let mut uri = String::new();
            let mut parameters = vec![];
            let mut function_hash = String::new();
            let mut router_variable = None;

            m.captures.iter().for_each(|capture| {
                let value =
                    params.code[capture.node.start_byte()..capture.node.end_byte()].to_string();
                match query.capture_names()[capture.index as usize] {
                    "function.name" => function_name = value,
                    "http.method" => http_method = value,
                    "http.uri" => uri = value.trim_matches('"').to_string(),
                    "router.variable" => router_variable = Some(value),
                    "function.params" => {
                        let p = parse_parameters(&value);
                        parameters.extend(p);
                    }
                    "function" => {
                        function_hash = statix::strings::hash_text(&value);
                    }
                    _ => {}
                }
            });

            endpoints.push(Endpoint {
                function_name,
                function_hash,
                http_method: http_method.parse().unwrap_or(HttpMethod::GET),
                parameters,
                uri,
                file_path: params.file_name.unwrap_or_default().to_string(),
                router_variable,
            });
        });
        endpoints
    }
}
