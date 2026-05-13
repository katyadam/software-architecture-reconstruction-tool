use std::sync::OnceLock;

use models::{Argument, CallStatement, source_code::SourceSpan};
use tree_sitter::{Query, QueryCursor, StreamingIterator, Tree};

use crate::{
    extraction::{
        calls::source_span::last_class_first_function_span,
        enclosing_lookup::{
            get_enclosing_node_by_kind, get_field_string_from_node, get_hashed_node_value,
        },
        extractor::Extractor,
        queries::CALL_STATEMENTS_QUERY,
    },
    parsing::arguments::parse_call_arguments,
};

pub struct CallStatementsExtractor;
impl Extractor<CallStatement> for CallStatementsExtractor {
    fn query(&self) -> &'static Query {
        static QUERY: OnceLock<Query> = OnceLock::new();

        QUERY.get_or_init(|| {
            Query::new(&tree_sitter_java::LANGUAGE.into(), CALL_STATEMENTS_QUERY)
                .expect("Failed to compile Java Calls Query")
        })
    }

    fn extract(&self, code: &str, tree: &Tree, _file_name: &str) -> Vec<CallStatement> {
        let query = self.query();
        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(query, tree.root_node(), code.as_bytes());
        let mut calls = Vec::new();

        while let Some(m) = matches.next() {
            let mut invoked_on: Option<String> = None;
            let mut arguments: Vec<Argument> = Vec::new();
            let mut args_string: String = "".to_string();
            let mut explicit_constructor_call: Option<String> = None;
            let mut name: Option<String> = None;
            let mut enclosing_function_name: Option<String> = None;
            let mut enclosing_class_name: Option<String> = None;
            let mut enclosing_function_hash: Option<String> = None;
            let mut is_self_invoke = false;
            let mut is_super_invoke = false;
            let mut source_span = SourceSpan::default();

            for capture in m.captures {
                let capture_text =
                    &code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();
                match query.capture_names()[capture.index as usize] {
                    "invoked_on" => {
                        if value == "this" {
                            is_self_invoke = true;
                        }
                        if value == "super" {
                            is_super_invoke = true;
                        }
                        invoked_on = Some(value);
                    }
                    "args" => {
                        arguments = parse_call_arguments(capture.node, code);
                        args_string = value;
                    }
                    "name" => name = Some(value),
                    "explicit_constructor_call" => {
                        if value.starts_with("this") {
                            is_self_invoke = true;
                        }
                        if value.starts_with("super") {
                            is_super_invoke = true;
                        }
                        explicit_constructor_call = Some(value);
                    }
                    "call" => {
                        source_span = last_class_first_function_span(&capture.node, code);

                        let n = ["method_declaration", "constructor_declaration"]
                            .iter()
                            .find_map(|kind| get_enclosing_node_by_kind(capture.node, kind));

                        if let Some(n) = n
                            && let (possible_ftype, Some(fname), Some(params)) = (
                                get_field_string_from_node(n, "type", code),
                                get_field_string_from_node(n, "name", code),
                                get_field_string_from_node(n, "parameters", code),
                            )
                        {
                            if let Some(ftype) = possible_ftype {
                                enclosing_function_name = Some(ftype + " " + &fname + &params);
                            } else {
                                enclosing_function_name = Some(fname + &params);
                            }
                            enclosing_function_hash = Some(get_hashed_node_value(n, code));
                        }

                        let n = [
                            "class_declaration",
                            "interface_declaration",
                            "record_declaration",
                            "enum_declaration",
                        ]
                        .iter()
                        .find_map(|kind| get_enclosing_node_by_kind(capture.node, kind));
                        if let Some(n) = n {
                            enclosing_class_name = get_field_string_from_node(n, "name", code);
                        }
                    }
                    _ => (),
                }
            }
            let joined_name = [
                invoked_on.unwrap_or_default(),
                name.unwrap_or_default(),
                explicit_constructor_call.unwrap_or_default(),
            ]
            .join(".");

            let joined_trimmed_name = joined_name.trim_matches('.');

            calls.push(CallStatement {
                function_name: joined_trimmed_name.to_string()
                    + &statix::strings::normalize_whitespace(&args_string),
                arguments,
                enclosing_function_name,
                enclosing_class_name,
                enclosing_function_hash,
                is_self_invoke,
                is_super_invoke,
                invoked_on: None,
                source_span,
            });
        }
        calls
    }
}
