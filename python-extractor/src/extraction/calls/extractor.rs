use std::sync::OnceLock;

use log::warn;
use models::{Argument, CallStatement};
use sha2::{Digest, Sha256};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use crate::extraction::{
    calls::PythonCallStatement,
    extractor::{ExtractParams, Extractor},
    queries::CALL_QUERY,
};

fn get_node_text<'a>(node: Node, code: &'a str) -> &'a str {
    &code[node.start_byte()..node.end_byte()]
}

fn find_enclosing_information(
    mut node: Node,
    code: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let mut function_node: Option<Node> = None;
    let mut class_node: Option<Node> = None;

    while let Some(parent) = node.parent() {
        match parent.kind() {
            "function_definition" if function_node.is_none() => function_node = Some(parent),
            "class_definition" if class_node.is_none() => class_node = Some(parent),
            _ => {}
        }

        // Optimization: stop if we found both
        if function_node.is_some() && class_node.is_some() {
            break;
        }
        node = parent;
    }

    let mut function_name_full = None;
    let mut function_hash = None;

    if let Some(f_node) = function_node {
        let name = f_node
            .child_by_field_name("name")
            .map(|n| get_node_text(n, code))
            .unwrap_or("unknown");

        let params = f_node
            .child_by_field_name("parameters")
            .map(|n| get_node_text(n, code))
            .unwrap_or("()");

        let return_type = f_node
            .child_by_field_name("return_type")
            .map(|n| get_node_text(n, code))
            .unwrap_or("Any");

        function_name_full = Some(format!("{}{} -> {}", name, params, return_type));

        let mut hasher = Sha256::new();
        hasher.update(get_node_text(f_node, code).as_bytes());
        function_hash = Some(format!("{:x}", hasher.finalize()));
    }

    // 3. Process Class Information
    let class_name = class_node
        .and_then(|n| n.child_by_field_name("name"))
        .map(|n| get_node_text(n, code).to_string());

    (function_name_full, class_name, function_hash)
}

pub struct CallsExtractor;

impl Extractor for CallsExtractor {
    type Item<'a> = PythonCallStatement<'a>;

    fn query(&self) -> &'static Query {
        static QUERY: OnceLock<Query> = OnceLock::new();

        QUERY.get_or_init(|| {
            Query::new(&tree_sitter_python::LANGUAGE.into(), CALL_QUERY)
                .expect("Failed to compile Python Calls Query")
        })
    }

    fn extract<'a>(&self, params: ExtractParams<'a>) -> Vec<Self::Item<'a>> {
        let query = self.query();

        let mut query_cursor = QueryCursor::new();
        let mut matches =
            query_cursor.matches(query, params.tree.root_node(), params.code.as_bytes());
        let mut call_statements: Vec<PythonCallStatement> = vec![];
        while let Some(m) = matches.next() {
            let mut arguments: Vec<Argument> = vec![];
            let mut function_name = String::new();
            let mut enclosing_function_name: Option<String> = None;
            let mut enclosing_class_name: Option<String> = None;
            let mut enclosing_function_hash: Option<String> = None;
            let mut call_node: Option<Node> = None;

            m.captures.iter().for_each(|capture| {
                let capture_text =
                    &params.code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();
                match query.capture_names()[capture.index as usize] {
                    "call.ident" => function_name = value,
                    "call.args" => {
                        let trimmed = if value.starts_with('(') && value.ends_with(')') {
                            &value[1..value.len() - 1]
                        } else {
                            value.as_str()
                        };
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
                                            assigned_variable: spl.first().unwrap().to_string(),
                                            value: spl.get(1).unwrap().to_string(),
                                            datatype: "any".to_string(),
                                        }
                                    } else {
                                        Argument {
                                            assigned_variable: "".to_string(),
                                            value: arg.to_string(),
                                            datatype: "any".to_string(),
                                        }
                                    }
                                })
                                .collect()
                        };
                    }
                    "call" => {
                        call_node = Some(capture.node);
                        (
                            enclosing_function_name,
                            enclosing_class_name,
                            enclosing_function_hash,
                        ) = find_enclosing_information(capture.node, params.code);
                    }
                    _ => {}
                }
            });

            let call_statement = CallStatement {
                function_name: function_name.clone(),
                arguments,
                enclosing_function_name,
                enclosing_class_name,
                enclosing_function_hash,
                is_self_invoke: function_name.starts_with("self"),
                is_super_invoke: false,
                invoked_on: None,
            };

            if let Some(node) = call_node {
                call_statements.push(PythonCallStatement {
                    call_statement,
                    node,
                });
            } else {
                warn!("Call Statement: {call_statement:#?} does not have Node!");
            }
        }

        call_statements
    }
}
