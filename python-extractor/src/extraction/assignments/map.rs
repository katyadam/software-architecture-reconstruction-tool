use std::collections::HashMap;

use log::warn;
use models::{Assignment, AssignmentKey, Scope};
use tree_sitter::{Query, QueryCursor, StreamingIterator, Tree};

use crate::extraction::queries::ASSINGMENTS_QUERY;

pub fn get_assignments_map(tree: &Tree, code: &str) -> HashMap<AssignmentKey, Assignment> {
    let query = Query::new(&tree_sitter_python::LANGUAGE.into(), ASSINGMENTS_QUERY).unwrap();
    let mut query_cursor = QueryCursor::new();
    let matches = query_cursor.matches(&query, tree.root_node(), code.as_bytes());
    let mut assignments_map: HashMap<AssignmentKey, Assignment> = HashMap::new();
    matches.for_each(|m| {
        let mut function_name: Option<String> = None;
        let mut function_params: Option<String> = None;
        let mut variable_name = String::new();
        let mut variable_value = String::new();

        m.captures.into_iter().for_each(|capture| {
            let capture_text = &code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
            let value = String::from_utf8_lossy(capture_text).to_string();
            match query.capture_names()[capture.index as usize] {
                "function.name" => function_name = Some(value),
                "function.params" => function_params = Some(value),
                "variable" => variable_name = value,
                "value" => variable_value = value,
                _ => {}
            }
        });
        let scope = match (function_name, function_params) {
            (Some(func_name), Some(params)) => Scope::Function(func_name + &params),
            (None, None) => Scope::Global,
            (None, Some(_)) => {
                warn!("Non compatible assignment. The assignment has function params but not function name!");
                Scope::Global
            },
            (Some(_), None) => {
                warn!("Non compatible assignment. The assignment has function name but not function params!");
                Scope::Global
            },
        };

        let new_assignment = Assignment {
            variable_name: variable_name.clone(),
            value: variable_value,
        };

        let assignment_key = AssignmentKey {
            scope: scope,
            variable_name: variable_name,
        };

        assignments_map.insert(assignment_key, new_assignment);
    });
    assignments_map
}

pub fn get_assignment(
    function_name: &String,
    variable_name: &String,
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) -> Option<Assignment> {
    let assignment_func = assignments_map.get(&AssignmentKey {
        scope: Scope::Function(function_name.clone()),
        variable_name: variable_name.clone(),
    });
    let assignment_global = assignments_map.get(&AssignmentKey {
        scope: Scope::Global,
        variable_name: variable_name.clone(),
    });

    match (assignment_func, assignment_global) {
        (Some(a1), Some(_)) => Some(a1.clone()),
        (Some(a1), None) => Some(a1.clone()),
        (None, Some(a2)) => Some(a2.clone()),
        (None, None) => None,
    }
}
