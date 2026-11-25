use std::sync::OnceLock;

use crate::{
    extraction::{
        endpoints::annotations::{
            annotation_name, apply_annotations, are_annotations_specifying_endpoint, extract_uris,
            get_annotations_from_modifiers,
        },
        extractor::Extractor,
        queries::ENDPOINTS_QUERY,
    },
    parsing::parameters::parse_callable_params,
};
use models::{Endpoint, HttpMethod};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator, Tree};

pub struct EndpointsExtractor;
impl Extractor<Endpoint> for EndpointsExtractor {
    fn query(&self) -> &'static Query {
        static QUERY: OnceLock<Query> = OnceLock::new();

        QUERY.get_or_init(|| {
            Query::new(&tree_sitter_java::LANGUAGE.into(), ENDPOINTS_QUERY)
                .expect("Failed to compile Java Endpoints Query")
        })
    }

    fn extract(&self, code: &str, tree: &Tree, file_name: &str) -> Vec<Endpoint> {
        let query = self.query();
        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(query, tree.root_node(), code.as_bytes());
        let mut endpoints = Vec::new();

        while let Some(m) = matches.next() {
            let mut annotations: Vec<String> = Vec::new();
            let mut return_type: Option<String> = None;
            let mut callable_name: Option<String> = None;
            let mut stringified_params: Option<String> = None;
            let mut function_hash: Option<String> = None;
            let mut prefixes: Option<Vec<String>> = None;
            for capture in m.captures {
                let capture_text =
                    &code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();
                match query.capture_names()[capture.index as usize] {
                    "return_type" => return_type = Some(value),
                    "callable_name" => callable_name = Some(value),
                    "callable_params" => stringified_params = Some(value),
                    "modifiers" => annotations = get_annotations_from_modifiers(capture.node, code),
                    "callable" => {
                        function_hash = Some(statix::strings::hash_text(&value));
                        prefixes = class_prefixes(capture.node, code);
                    }
                    _ => (),
                }
            }
            let Some(prefixes) = prefixes else { continue };
            if function_hash.is_none() || !are_annotations_specifying_endpoint(&annotations) {
                continue;
            }
            let normalized_stringified_params =
                statix::strings::normalize_whitespace(&stringified_params.unwrap_or_default());

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
                router_variable: None,
            };
            apply_annotations(&mut endpoint, &annotations);
            for prefix in prefixes {
                let mut e = endpoint.clone();
                e.uri = format!("{}{}", prefix, endpoint.uri);
                endpoints.push(e);
            }
        }
        endpoints
    }
}

/// Route prefixes of the type that declares `method`, taken only from its
/// `@RequestMapping`/`@Path`. `None` when the type is a `@FeignClient` (a
/// client, not a server endpoint). No route annotation -> `[""]`.
fn class_prefixes(method: Node, code: &str) -> Option<Vec<String>> {
    let mut node = method.parent();
    while let Some(n) = node {
        if matches!(
            n.kind(),
            "class_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "record_declaration"
                | "object_creation_expression" // anonymous class: no annotations
        ) {
            break;
        }
        node = n.parent();
    }
    let annotations = node
        .and_then(|n| n.children(&mut n.walk()).find(|c| c.kind() == "modifiers"))
        .map(|m| get_annotations_from_modifiers(m, code))
        .unwrap_or_default();

    if annotations
        .iter()
        .any(|a| annotation_name(a) == "FeignClient")
    {
        return None;
    }
    let prefixes: Vec<String> = annotations
        .iter()
        .find(|a| matches!(annotation_name(a), "RequestMapping" | "Path"))
        .map(|a| extract_uris(a))
        .unwrap_or_default();
    Some(if prefixes.is_empty() {
        vec![String::new()]
    } else {
        prefixes
    })
}
