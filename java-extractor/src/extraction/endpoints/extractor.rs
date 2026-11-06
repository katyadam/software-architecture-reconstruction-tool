use std::sync::OnceLock;

use crate::{
    extraction::{
        endpoints::annotations::{
            are_annotations_specifying_endpoint, distinct_capped, expand_annotations,
            first_string_literal, get_annotations_from_modifiers, is_named, path_values,
        },
        extractor::Extractor,
        queries::ENDPOINTS_QUERY,
    },
    parsing::parameters::parse_callable_params,
};
use models::{Endpoint, HttpMethod, endpoints::PrefixExpr};
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
            let mut type_annotations: Option<(&str, Vec<String>)> = None;
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
                        // ponytail: re-reads the type's annotations per method (methods x
                        // annotation size); cache per type node id if that ever shows up.
                        type_annotations = enclosing_type_annotations(capture.node, code);
                    }
                    _ => (),
                }
            }
            if function_hash.is_none() || !are_annotations_specifying_endpoint(&annotations) {
                continue;
            }
            let (type_kind, type_annotations) = type_annotations.unwrap_or_default();
            if type_kind == "interface_declaration"
                && type_annotations.iter().any(|a| is_named(a, "@FeignClient"))
            {
                continue;
            }
            let normalized_stringified_params =
                statix::strings::normalize_whitespace(&stringified_params.unwrap_or_default());

            let endpoint = Endpoint {
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
                enclosing_function: None,
                route_prefixes: class_prefixes(&type_annotations, file_name),
            };
            endpoints.extend(expand_annotations(endpoint, &annotations));
        }
        endpoints
    }
}

const TYPE_KINDS: &[&str] = &[
    "class_declaration",
    "interface_declaration",
    "enum_declaration",
    "record_declaration",
];

/// Kind and annotations of the nearest type declaration enclosing `node`.
fn enclosing_type_annotations<'a>(node: Node<'a>, code: &str) -> Option<(&'a str, Vec<String>)> {
    let mut curr = node.parent();
    while let Some(n) = curr {
        // Anonymous class: its methods get no prefix from the outer type.
        if n.kind() == "class_body"
            && n.parent().map(|p| p.kind()) == Some("object_creation_expression")
        {
            return None;
        }
        if TYPE_KINDS.contains(&n.kind()) {
            let modifiers = n.children(&mut n.walk()).find(|c| c.kind() == "modifiers");
            let annotations = modifiers
                .map(|m| get_annotations_from_modifiers(m, code))
                .unwrap_or_default();
            return Some((n.kind(), annotations));
        }
        curr = n.parent();
    }
    None
}

/// One single-element chain per `@RequestMapping` / `@Path` value of the type.
/// A literal is re-quoted (drops comments around it); `Paths.BASE` -> `BASE`, as pass 2
/// keys constants by bare name; anything else stays as written and resolves to nothing.
fn class_prefixes(type_annotations: &[String], file_name: &str) -> Vec<Vec<PrefixExpr>> {
    let exprs = type_annotations
        .iter()
        .filter(|a| is_named(a, "@RequestMapping") || is_named(a, "@Path"))
        .flat_map(|a| path_values(a))
        .map(|v| {
            if let Some(literal) = sole_literal(v) {
                return format!("\"{literal}\"");
            }
            let qualified = v
                .split('.')
                .all(|s| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_'));
            match v.rsplit('.').next() {
                Some(last) if qualified => last.to_string(),
                _ => v.to_string(),
            }
        });
    distinct_capped(exprs, "class prefixes")
        .into_iter()
        .map(|expr| {
            vec![PrefixExpr {
                file_path: file_name.to_string(),
                expr,
            }]
        })
        .collect()
}

/// Contents of `v` when it is one string literal plus only comments
/// (`/* v1 */ "/api"`); `None` for `X + "/a"`.
fn sole_literal(v: &str) -> Option<&str> {
    let literal = first_string_literal(v)?;
    let start = v.find('"')?;
    let rest = format!("{}{}", &v[..start], &v[start + literal.len() + 2..]);
    let mut rest = rest.trim();
    while !rest.is_empty() {
        rest = if let Some(r) = rest.strip_prefix("/*") {
            r.split_once("*/")?.1.trim()
        } else {
            let r = rest.strip_prefix("//")?;
            r.split_once('\n').map_or("", |(_, after)| after.trim())
        };
    }
    Some(literal)
}
