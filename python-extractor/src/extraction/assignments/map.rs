use std::collections::HashMap;

use log::warn;
use models::{Assignment, AssignmentKey, Scope};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator, Tree};

use crate::extraction::callables::parser::parse_parameters;
use crate::extraction::queries::ASSINGMENTS_QUERY;

fn find_enclosing_function(mut node: Node, code: &str) -> Option<(String, String)> {
    while let Some(parent) = node.parent() {
        if parent.kind() == "function_definition" {
            let name_node = parent.child_by_field_name("name")?;
            let params_node = parent.child_by_field_name("parameters")?;

            let name = &code[name_node.start_byte()..name_node.end_byte()];
            let params = &code[params_node.start_byte()..params_node.end_byte()];

            return Some((name.to_string(), params.to_string()));
        }
        node = parent;
    }

    None
}

pub fn get_assignments_map(tree: &Tree, code: &str) -> HashMap<AssignmentKey, Assignment> {
    let query = Query::new(&tree_sitter_python::LANGUAGE.into(), ASSINGMENTS_QUERY).unwrap();
    let mut query_cursor = QueryCursor::new();
    let matches = query_cursor.matches(&query, tree.root_node(), code.as_bytes());
    let mut assignments_map: HashMap<AssignmentKey, Assignment> = HashMap::new();
    matches.for_each(|m| {
        let mut variable_name = String::new();
        let mut variable_value = String::new();
        let mut variable_type = String::new();
        let mut function_name = String::new();
        let mut function_params = String::new();

        m.captures.iter().for_each(|capture| {
            let node = capture.node;
            let capture_text = &code.as_bytes()[node.start_byte()..node.end_byte()];
            let value = String::from_utf8_lossy(capture_text).to_string();

            match query.capture_names()[capture.index as usize] {
                "variable" => variable_name = value,
                "type" => variable_type = value,
                "value" => variable_value = value,
                "function.name" => function_name = value,
                "function.params" => function_params = value,
                _ => {}
            }

            // After getting assignment fields:
            let (func_name, func_params) = match find_enclosing_function(node, code) {
                Some((name, params)) => (Some(name), Some(params)),
                None => (None, None),
            };

            let scope = get_scope(func_name.as_deref(), func_params.as_deref());

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
                assignments_map.insert(assignment_key, new_assignment);
            }
        });
        let functions_params_assignments =
            create_assignments_from_function_params(&function_params, &function_name);
        assignments_map.extend(functions_params_assignments);
    });
    assignments_map
}

fn get_scope(function_name: Option<&str>, function_params: Option<&str>) -> Scope {
    match (function_name, function_params) {
        (Some(func_name), Some(params)) => Scope::Function(func_name.to_string() + params),
        (None, None) => Scope::Global,
        (None, Some(_)) => {
            warn!(
                "Non compatible assignment. The assignment has function params but not function name!"
            );
            Scope::Global
        }
        (Some(_), None) => {
            warn!(
                "Non compatible assignment. The assignment has function name but not function params!"
            );
            Scope::Global
        }
    }
}

fn create_assignments_from_function_params(
    function_params: &str,
    function_name: &str,
) -> Vec<(AssignmentKey, Assignment)> {
    let scope = get_scope(Some(function_name), Some(function_params));
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
