use models::{HttpMethod, RestCall};
use sha2::{Digest, Sha256};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::extraction::{
    common::{clean_formatted_python_string, extract_function_arguments},
    extractor::{ExtractParams, Extractor},
    queries::RESTCALLS_QUERY,
};

pub struct RestcallsExtractor;

impl Extractor<RestCall> for RestcallsExtractor {
    fn extract(&self, params: ExtractParams) -> Vec<RestCall> {
        let query = Query::new(&tree_sitter_python::LANGUAGE.into(), RESTCALLS_QUERY).unwrap();
        let mut query_cursor = QueryCursor::new();
        let matches = query_cursor.matches(&query, params.tree.root_node(), params.code.as_bytes());
        let mut restcalls: Vec<RestCall> = vec![];
        matches.for_each(|m| {
            let mut function_name = String::new();
            let mut function_parameters = String::new();
            let mut call_arguments = vec![];
            let mut http_method = String::new();
            let mut target_uri = String::new();
            let mut function_hash = String::new();

            m.captures.into_iter().for_each(|capture| {
                let capture_text =
                    &params.code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();
                match query.capture_names()[capture.index as usize] {
                    "http.method" => http_method = value,
                    "uri" => {
                        if value.starts_with("f\"") {
                            target_uri = clean_formatted_python_string(value);
                        } else {
                            target_uri = value;
                        }
                    }
                    "function.name" => function_name = value,
                    "function.parameters" => function_parameters = value,
                    "call.args" => {
                        let param_names = extract_function_arguments(capture.node, &params.code);
                        call_arguments.extend(param_names);
                    }
                    "function" => {
                        let mut hasher = Sha256::new();
                        hasher.update(value.as_bytes());
                        function_hash = format!("{:x}", hasher.finalize());
                    }
                    _ => {}
                }
            });
            let rest_call = RestCall {
                function_name: function_name.clone() + &function_parameters,
                function_hash,
                call_arguments,
                http_method: http_method.parse().unwrap_or(HttpMethod::GET),
                target_uri: target_uri.clone(),
                file_path: params.file_name.unwrap_or_default().to_string(),
            };

            restcalls.push(rest_call);
        });

        restcalls
    }
}
