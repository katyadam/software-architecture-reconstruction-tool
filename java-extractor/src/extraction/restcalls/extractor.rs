use models::RestCall;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use crate::{
    extraction::{
        extractor::Extractor,
        queries::RESTCALLS_QUERY,
        restcalls::{
            enclosing_lookup::get_enclosing_function_signature_and_hash,
            identification::{get_identification_strategy, strategy::Strategy},
        },
    },
    parsing::arguments::parse_call_arguments,
};

pub struct RestCallsExtractor;
impl Extractor<RestCall> for RestCallsExtractor {
    fn extract(&self, code: &str, tree: &tree_sitter::Tree, file_name: &str) -> Vec<RestCall> {
        let query = Query::new(&tree_sitter_java::LANGUAGE.into(), RESTCALLS_QUERY).unwrap();
        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(&query, tree.root_node(), code.as_bytes());
        let mut restcalls = Vec::new();

        while let Some(m) = matches.next() {
            let mut invoked_on: Option<String> = None;
            let mut callable_name: Option<String> = None;
            let mut invoke_node: Option<Node> = None;
            let mut arguments_node: Option<Node> = None;
            for capture in m.captures {
                let capture_text =
                    &code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();
                match query.capture_names()[capture.index as usize] {
                    "invoked_on" => invoked_on = Some(value),
                    "invoked_callable_name" => callable_name = Some(value),
                    "arguments" => arguments_node = Some(capture.node),
                    "invoke" => invoke_node = Some(capture.node),
                    _ => (),
                }
            }
            if let (Some(inv_node), Some(args_node), Some(callable_name)) =
                (invoke_node, arguments_node, callable_name)
            {
                let call_args = parse_call_arguments(args_node, code);
                let strategy =
                    match get_identification_strategy(invoked_on, &callable_name, &call_args) {
                        Some(strategy) => strategy,
                        None => continue,
                    };
                let http_method = match strategy.identify_http_method() {
                    Some(method) => method,
                    None => continue,
                };

                let target_uri = match strategy.identify_target_uri() {
                    Some(uri) => uri,
                    None => continue,
                };
                let (function_name, function_hash) =
                    get_enclosing_function_signature_and_hash(inv_node, code);

                restcalls.push(RestCall {
                    function_name: function_name,
                    function_hash: function_hash,
                    call_arguments: parse_call_arguments(args_node, code),
                    http_method: http_method,
                    target_uri,
                    file_path: file_name.to_string(),
                });
            }
        }

        restcalls
    }
}
