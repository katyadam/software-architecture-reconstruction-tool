use std::sync::OnceLock;

use crate::extraction::callables::parser::parse_parameters;
use crate::extraction::calls::extractor::CallsExtractor;
use crate::extraction::endpoints::{EndpointStrategy, PythonEndpointStrategy};
use crate::extraction::extractor::{ExtractParams, Extractor};
use crate::extraction::queries::ENDPOINTS_QUERY;
use models::{Endpoint, HttpMethod};
use tree_sitter::StreamingIterator;
use tree_sitter::{Query, QueryCursor};

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
            let mut decorator = String::new();
            let mut decorator_args = String::new();

            m.captures.iter().for_each(|capture| {
                let value =
                    params.code[capture.node.start_byte()..capture.node.end_byte()].to_string();
                match query.capture_names()[capture.index as usize] {
                    "function.name" => function_name = value,
                    "http.method" => http_method = value,
                    "http.uri" => uri = normalize_python_route(&clean_python_string(&value)),
                    "router.variable" => router_variable = Some(value),
                    "decorator" => decorator = value,
                    "decorator.args" => decorator_args = value,
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

            for method in endpoint_methods(&http_method, &decorator, &decorator_args) {
                endpoints.push(Endpoint {
                    function_name: function_name.clone(),
                    function_hash: function_hash.clone(),
                    http_method: method,
                    parameters: parameters.clone(),
                    uri: uri.clone(),
                    file_path: params.file_name.unwrap_or_default().to_string(),
                    router_variable: router_variable.clone(),
                });
            }
        });

        endpoints.extend(extract_django_urlpatterns(params));
        endpoints
    }
}

impl EndpointStrategy for PythonEndpointStrategy {
    fn extract<'a>(&self, params: ExtractParams<'a>) -> Vec<Endpoint> {
        <Self as Extractor>::extract(self, params)
    }
}

fn extract_django_urlpatterns(params: ExtractParams<'_>) -> Vec<Endpoint> {
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
        uri: normalize_python_route(&route),
        file_path: params.file_name.unwrap_or_default().to_string(),
        router_variable: Some("urlpatterns".to_string()),
    })
}

fn endpoint_methods(http_method: &str, decorator: &str, decorator_args: &str) -> Vec<HttpMethod> {
    if http_method == "route" {
        let methods = flask_route_methods(decorator);
        if methods.is_empty() {
            return vec![HttpMethod::GET];
        }
        return methods;
    }
    if http_method == "api_view" {
        return python_http_method_list(decorator_args);
    }

    http_method
        .parse()
        .ok()
        .into_iter()
        .collect::<Vec<HttpMethod>>()
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

fn python_http_method_list(raw: &str) -> Vec<HttpMethod> {
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

fn django_view_name(view: &str) -> String {
    let cleaned = view.trim();
    let without_call = cleaned.split('(').next().unwrap_or(cleaned);
    without_call
        .rsplit('.')
        .find(|part| !part.is_empty() && *part != "as_view")
        .unwrap_or(without_call)
        .to_string()
}

fn clean_python_string(raw: &str) -> String {
    statix::strings::clean_python_string(raw)
}

fn normalize_python_route(route: &str) -> String {
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
