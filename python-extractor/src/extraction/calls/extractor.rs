use models::{Argument, CallStatement};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use crate::extraction::{
    extractor::{ExtractParams, Extractor},
    queries::CALL_QUERY,
};

fn find_enclosing_information(mut node: Node, code: &str) -> (Option<String>, Option<String>) {
    let mut function_node: Option<Node> = None;
    let mut class_node: Option<Node> = None;
    let mut function_name: Option<String> = None;
    let mut class_name: Option<String> = None;
    while let Some(parent) = node.parent() {
        if !(function_node.is_none() || class_node.is_none()) {
            break;
        }
        if parent.kind() == "function_definition" && function_node.is_none() {
            function_node = Some(parent);
        }
        if parent.kind() == "class_definition" && class_node.is_none() {
            class_node = Some(parent);
        }
        node = parent;
    }

    if let Some(name_node) = function_node.and_then(|n| n.child_by_field_name("name")) {
        let name_bytes = &code.as_bytes()[name_node.start_byte()..name_node.end_byte()];
        function_name = Some(String::from_utf8_lossy(name_bytes).to_string());
    }

    if let Some(name_node) = class_node.and_then(|n| n.child_by_field_name("name")) {
        let name_bytes = &code.as_bytes()[name_node.start_byte()..name_node.end_byte()];
        class_name = Some(String::from_utf8_lossy(name_bytes).to_string());
    }

    (function_name, class_name)
}

pub struct CallsExtractor;

impl Extractor<CallStatement> for CallsExtractor {
    fn extract(&self, params: ExtractParams) -> Vec<CallStatement> {
        let query = Query::new(&tree_sitter_python::LANGUAGE.into(), CALL_QUERY).unwrap();

        let mut query_cursor = QueryCursor::new();
        let mut matches =
            query_cursor.matches(&query, params.tree.root_node(), params.code.as_bytes());
        let mut call_statements: Vec<CallStatement> = vec![];
        while let Some(m) = matches.next() {
            let mut arguments: Vec<Argument> = vec![];
            let mut function_name = String::new();
            let mut enclosing_func_name: Option<String> = None;
            let mut enclosing_class_name: Option<String> = None;

            m.captures.into_iter().for_each(|capture| {
                let capture_text =
                    &params.code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();
                match query.capture_names()[capture.index as usize] {
                    "call.ident" => function_name = value,
                    "call.args" => {
                        let trimmed = value.trim_matches(|c| c == '(' || c == ')');
                        arguments = if trimmed.is_empty() {
                            vec![]
                        } else {
                            trimmed
                                .split(',')
                                .map(str::trim)
                                .map(|arg| {
                                    if arg.contains("=") {
                                        let spl: Vec<&str> = arg.split("=").collect();
                                        Argument {
                                            assigned_variable: spl.get(0).unwrap().to_string(),
                                            value: spl.get(1).unwrap().to_string(),
                                        }
                                    } else {
                                        Argument {
                                            assigned_variable: "".to_string(),
                                            value: arg.to_string(),
                                        }
                                    }
                                })
                                .collect()
                        };
                    }
                    "call" => {
                        (enclosing_func_name, enclosing_class_name) =
                            find_enclosing_information(capture.node, params.code);
                    }
                    _ => {}
                }
            });

            let new_call_statement = CallStatement {
                function_name: function_name,
                arguments: arguments,
                enclosing_function_name: enclosing_func_name,
                enclosing_class_name: enclosing_class_name,
            };

            call_statements.push(new_call_statement);
        }

        call_statements
    }
}
