use sha2::{Digest, Sha256};
use tree_sitter::{Query, Tree};

use crate::extraction::{
    endpoints::annotations::{apply_annotations, extract_uri},
    extractor::Extractor,
    parser::{normalize_whitespace, parse_callable_params},
    queries::ENDPOINTS_QUERY,
};
use models::{Endpoint, HttpMethod};
use tree_sitter::{QueryCursor, StreamingIterator};

pub struct EndpointsExtractor;
impl Extractor<Endpoint> for EndpointsExtractor {
    fn extract(&self, code: &str, tree: &Tree, file_name: &str) -> Vec<Endpoint> {
        let query = Query::new(&tree_sitter_java::LANGUAGE.into(), ENDPOINTS_QUERY).unwrap();
        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(&query, tree.root_node(), code.as_bytes());
        let mut endpoints = Vec::new();
        let mut shared_uri: Option<String> = None;

        while let Some(m) = matches.next() {
            let mut annotations: Vec<String> = Vec::new();
            let mut return_type: Option<String> = None;
            let mut callable_name: Option<String> = None;
            let mut stringified_params: Option<String> = None;
            let mut function_hash: Option<String> = None;
            for capture in m.captures {
                let capture_text =
                    &code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();
                match query.capture_names()[capture.index as usize] {
                    "class_annotation" => shared_uri = extract_uri(&value),
                    "return_type" => return_type = Some(value),
                    "callable_name" => callable_name = Some(value),
                    "callable_params" => stringified_params = Some(value),
                    "annotation" => annotations.push(value),
                    "callable" => {
                        let mut hasher = Sha256::new();
                        hasher.update(value.as_bytes());
                        function_hash = Some(format!("{:x}", hasher.finalize()));
                    }
                    _ => (),
                }
            }
            if function_hash.is_none() {
                continue;
            }
            let normalized_stringified_params =
                normalize_whitespace(&stringified_params.unwrap_or_default());

            let mut endpoint = Endpoint {
                function_name: return_type.unwrap_or_default()
                    + " "
                    + &callable_name.unwrap_or_default()
                    + &normalized_stringified_params.clone(),
                parameters: parse_callable_params(&normalized_stringified_params),
                function_hash: function_hash.unwrap_or_default(),
                http_method: HttpMethod::GET,
                uri: "".to_string(),
                file_path: file_name.to_string(),
            };

            apply_annotations(&mut endpoint, annotations);
            endpoints.push(endpoint);
        }
        println!("{shared_uri:?}");
        prepend_shared_uri(&mut endpoints, &shared_uri.unwrap_or_default());
        endpoints
    }
}

fn prepend_shared_uri(endpoints: &mut [Endpoint], shared_uri: &str) {
    endpoints
        .iter_mut()
        .for_each(|e| e.uri = format!("{}{}", shared_uri, e.uri));
}
