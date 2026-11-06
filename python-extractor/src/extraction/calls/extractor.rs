use std::sync::OnceLock;

use log::warn;
use models::{Argument, CallStatement};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use crate::extraction::{
    calls::{PythonCallStatement, source_span::last_class_last_function_span},
    common::find_enclosing_information,
    extractor::{ExtractParams, Extractor},
    queries::CALL_QUERY,
};

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
                let value =
                    params.code[capture.node.start_byte()..capture.node.end_byte()].to_string();
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
                                            datatype: None,
                                        }
                                    } else {
                                        Argument {
                                            assigned_variable: "".to_string(),
                                            value: arg.to_string(),
                                            datatype: None,
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

            if let Some(node) = call_node {
                let call_statement = CallStatement {
                    function_name: function_name.clone(),
                    arguments,
                    enclosing_function_name,
                    enclosing_class_name,
                    enclosing_function_hash,
                    is_self_invoke: function_name.starts_with("self"),
                    is_super_invoke: false,
                    invoked_on: None,
                    source_span: last_class_last_function_span(&node, params.code),
                    is_decorator: node
                        .parent()
                        .is_some_and(|parent| parent.kind() == "decorator"),
                };
                call_statements.push(PythonCallStatement {
                    call_statement,
                    node,
                });
            } else {
                warn!("Call Statement does not have its 'call' Node!");
            }
        }

        call_statements
    }
}
