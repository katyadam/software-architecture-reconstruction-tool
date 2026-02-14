use std::sync::OnceLock;

use log::warn;
use models::{Assignment, AssignmentKey, Scope};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use crate::extraction::{
    callables::parser::parse_parameters, extractor::Extractor, queries::ASSINGMENTS_QUERY,
};

pub struct AssignmentsExtractor;

impl Extractor for AssignmentsExtractor {
    type Item<'a> = (AssignmentKey, Assignment);

    fn query(&self) -> &'static tree_sitter::Query {
        static QUERY: OnceLock<Query> = OnceLock::new();

        QUERY.get_or_init(|| {
            Query::new(&tree_sitter_python::LANGUAGE.into(), ASSINGMENTS_QUERY)
                .expect("Failed to compile Python Assignments Query")
        })
    }

    fn extract<'a>(
        &self,
        params: crate::extraction::extractor::ExtractParams<'a>,
    ) -> Vec<Self::Item<'a>> {
        let query = self.query();

        let mut query_cursor = QueryCursor::new();
        let matches = query_cursor.matches(query, params.tree.root_node(), params.code.as_bytes());
        let mut assignments_map: Vec<(AssignmentKey, Assignment)> = Vec::new();
        matches.for_each(|m| {
            let mut variable_name = String::new();
            let mut variable_value = String::new();
            let mut variable_type = String::new();
            let mut function_name = String::new();
            let mut function_params = String::new();
            let mut function_return_type: Option<String> = None;

            m.captures.iter().for_each(|capture| {
                let node = capture.node;
                let capture_text = &params.code.as_bytes()[node.start_byte()..node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();

                match query.capture_names()[capture.index as usize] {
                    "variable" => variable_name = value,
                    "type" => variable_type = value,
                    "value" => variable_value = value,
                    "function.name" => function_name = value,
                    "function.params" => function_params = value,
                    "function.return_type" => function_return_type = Some(value),
                    _ => {}
                }

                // After getting assignment fields:
                let (func_name, func_params, func_return_type) =
                    match find_enclosing_function(node, params.code) {
                        Some((name, params, return_type)) => {
                            (Some(name), Some(params), Some(return_type))
                        }
                        None => (None, None, None),
                    };

                let scope = get_scope(
                    func_name.as_deref(),
                    func_params.as_deref(),
                    func_return_type.as_deref(),
                );

                let new_assignment = Assignment {
                    variable_name: variable_name.clone(),
                    variable_type: variable_type.clone(),
                    value: variable_value.clone(),
                };

                let assignment_key = AssignmentKey {
                    scope,
                    variable_name: variable_name.clone(),
                };

                if !variable_name.is_empty() {
                    assignments_map.push((assignment_key, new_assignment));
                }
            });
            let functions_params_assignments = create_assignments_from_function_params(
                &function_params,
                &function_name,
                &function_return_type.unwrap_or("Any".to_owned()),
            );
            assignments_map.extend(functions_params_assignments);
        });
        assignments_map
    }
}

fn find_enclosing_function(mut node: Node, code: &str) -> Option<(String, String, String)> {
    while let Some(parent) = node.parent() {
        if parent.kind() == "function_definition" {
            let name_node = parent.child_by_field_name("name")?;
            let params_node = parent.child_by_field_name("parameters")?;
            let return_type = parent
                .child_by_field_name("return_type")
                .map(|n| &code[n.start_byte()..n.end_byte()])
                .unwrap_or("Any");
            let name = &code[name_node.start_byte()..name_node.end_byte()];
            let params = &code[params_node.start_byte()..params_node.end_byte()];

            return Some((
                name.to_string(),
                params.to_string(),
                return_type.to_string(),
            ));
        }
        node = parent;
    }

    None
}

fn get_scope(
    function_name: Option<&str>,
    function_params: Option<&str>,
    function_return_type: Option<&str>,
) -> Scope {
    match (function_name, function_params, function_return_type) {
        (Some(func_name), Some(params), Some(ret)) => {
            Scope::Function(func_name.to_string() + params + " -> " + ret)
        }
        (Some(func_name), Some(params), None) => {
            Scope::Function(func_name.to_string() + params + " -> Any")
        }
        (None, None, None) => Scope::Global,
        (None, Some(_), _) => {
            warn!(
                "Non compatible assignment. The assignment has function params but not function name!"
            );
            Scope::Global
        }
        (Some(_), None, _) => {
            warn!(
                "Non compatible assignment. The assignment has function name but not function params!"
            );
            Scope::Global
        }
        (None, None, Some(_)) => {
            warn!("Non compatible assignment. The assignment has only return type!");
            Scope::Global
        }
    }
}

fn create_assignments_from_function_params(
    function_params: &str,
    function_name: &str,
    function_return_type: &str,
) -> Vec<(AssignmentKey, Assignment)> {
    let scope = get_scope(
        Some(function_name),
        Some(function_params),
        Some(function_return_type),
    );
    let mut assignments = Vec::new();
    parse_parameters(function_params).iter().for_each(|param| {
        let new_assignment = Assignment {
            variable_name: param.name.clone(),
            variable_type: param.datatype.clone().unwrap_or("any".to_string()),
            value: param.initial_value.clone().unwrap_or_default(),
        };

        let assignment_key = AssignmentKey {
            scope: scope.clone(),
            variable_name: param.name.clone(),
        };
        assignments.push((assignment_key, new_assignment));
    });
    assignments
}
