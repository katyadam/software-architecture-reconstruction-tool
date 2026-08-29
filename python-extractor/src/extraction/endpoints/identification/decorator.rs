use std::sync::OnceLock;

use crate::extraction::callables::parser::parse_parameters;
use crate::extraction::extractor::ExtractParams;
use crate::extraction::queries::ENDPOINTS_QUERY;
use models::Parameter;
use tree_sitter::StreamingIterator;
use tree_sitter::{Query, QueryCursor};

#[derive(Debug, Clone, Default)]
pub(crate) struct DecoratorEndpointMatch {
    pub function_name: String,
    pub function_hash: String,
    pub http_method: String,
    pub uri: String,
    pub parameters: Vec<Parameter>,
    pub router_variable: Option<String>,
    pub decorator: String,
    pub decorator_args: String,
}

pub(crate) fn decorator_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();

    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_python::LANGUAGE.into(), ENDPOINTS_QUERY)
            .expect("Failed to compile Python Endpoints Query")
    })
}

pub(crate) fn collect_decorator_matches(
    params: ExtractParams<'_>,
) -> Vec<DecoratorEndpointMatch> {
    let query = decorator_query();
    let mut query_cursor = QueryCursor::new();
    let matches = query_cursor.matches(query, params.tree.root_node(), params.code.as_bytes());
    let mut endpoints = Vec::new();

    matches.for_each(|query_match| {
        let mut endpoint = DecoratorEndpointMatch::default();

        query_match.captures.iter().for_each(|capture| {
            let value = params.code[capture.node.start_byte()..capture.node.end_byte()].to_string();
            match query.capture_names()[capture.index as usize] {
                "function.name" => endpoint.function_name = value,
                "http.method" => endpoint.http_method = value,
                "http.uri" => endpoint.uri = normalize_python_route(&clean_python_string(&value)),
                "router.variable" => endpoint.router_variable = Some(value),
                "decorator" => endpoint.decorator = value,
                "decorator.args" => endpoint.decorator_args = value,
                "function.params" => endpoint.parameters.extend(parse_parameters(&value)),
                "function" => endpoint.function_hash = statix::strings::hash_text(&value),
                _ => {}
            }
        });

        endpoints.push(endpoint);
    });

    endpoints
}

pub(crate) fn clean_python_string(raw: &str) -> String {
    statix::strings::clean_python_string(raw)
}

pub(crate) fn normalize_python_route(route: &str) -> String {
    let mut normalized = String::new();
    let mut chars = route.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '<' {
            normalized.push(ch);
            continue;
        }

        let mut param = String::new();
        for inner in chars.by_ref() {
            if inner == '>' {
                break;
            }
            param.push(inner);
        }
        let name = param.rsplit(':').next().unwrap_or(param.as_str()).trim();
        normalized.push('{');
        normalized.push_str(name);
        normalized.push('}');
    }
    normalized
}
