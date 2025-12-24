use models::{Callable, Namespace};
use sha2::{Digest, Sha256};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::{
    extraction::{
        callables::enclosing_lookup::get_enclosing_element_name, extractor::Extractor,
        queries::CALLABLES_QUERY,
    },
    parsing::parameters::{normalize_whitespace, parse_callable_params},
};

pub struct CallablesExtractor;

impl Extractor<Callable> for CallablesExtractor {
    fn extract(&self, code: &str, tree: &tree_sitter::Tree, file_name: &str) -> Vec<Callable> {
        let query = Query::new(&tree_sitter_java::LANGUAGE.into(), CALLABLES_QUERY).unwrap();
        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(&query, tree.root_node(), code.as_bytes());
        let mut callables = Vec::new();

        while let Some(m) = matches.next() {
            let mut name: Option<String> = None;
            let mut params_string: Option<String> = None;
            let mut return_type: Option<String> = None;
            let mut is_constructor = false;
            let mut hash: Option<String> = None;
            let mut namespace: Option<Namespace> = None;
            for capture in m.captures {
                let capture_text =
                    &code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();
                match query.capture_names()[capture.index as usize] {
                    "callable_name" => name = Some(value),
                    "callable_type" => return_type = Some(value),
                    "callable_params" => params_string = Some(value),
                    "callable" => {
                        let enclosing_element_name = get_enclosing_element_name(capture.node, code);
                        namespace = Some(Namespace::Class(enclosing_element_name));
                        let mut hasher = Sha256::new();
                        hasher.update(value.as_bytes());
                        hash = Some(format!("{:x}", hasher.finalize()));
                    }
                    "type_constructor" => is_constructor = true,
                    _ => (),
                }
            }

            let normalized_stringified_params =
                normalize_whitespace(&params_string.unwrap_or_default());

            let callable_name = return_type.clone().unwrap_or_default()
                + " "
                + &name.unwrap_or_default()
                + &normalized_stringified_params.clone();
            let callable_name = callable_name.trim();
            callables.push(Callable {
                name: callable_name.to_string(),
                signature: format!(
                    "{}/{}",
                    namespace.clone().unwrap_or_default().get_signature(),
                    callable_name
                ),
                namespace: namespace.unwrap_or_default(),
                parameters: parse_callable_params(&normalized_stringified_params),
                return_type,
                is_async: false,
                is_constructor,
                hash: hash.unwrap_or_default(),
                file_path: file_name.to_string(),
            });
        }
        callables
    }
}
