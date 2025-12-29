use std::collections::HashMap;

use models::{Assignment, AssignmentKey, Scope};
use tree_sitter::{Query, QueryCursor, StreamingIterator, Tree};

use crate::{
    extraction::{
        enclosing_lookup::{get_enclosing_node_by_kind, get_field_string_from_node},
        queries::ASSINGMENTS_QUERY,
    },
    parsing::parameters::parse_callable_params,
};

pub fn get_assignments_map(tree: &Tree, code: &str) -> HashMap<AssignmentKey, Assignment> {
    let query = Query::new(&tree_sitter_java::LANGUAGE.into(), ASSINGMENTS_QUERY).unwrap();
    let mut query_cursor = QueryCursor::new();
    let matches = query_cursor.matches(&query, tree.root_node(), code.as_bytes());
    let mut assignments_map: HashMap<AssignmentKey, Assignment> = HashMap::new();
    matches.for_each(|m| {
        let mut variable_name: Option<String> = None;
        let mut variable_value: Option<String> = None;
        let mut variable_type: Option<String> = None;
        let mut scope: Option<Scope> = None;

        let mut function_name: Option<String> = None;
        let mut params_string: Option<String> = None;
        m.captures.iter().for_each(|capture| {
            let capture_text = &code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
            let value = String::from_utf8_lossy(capture_text).to_string();

            match query.capture_names()[capture.index as usize] {
                "name" => variable_name = Some(value),
                "type" => variable_type = Some(value),
                "value" => variable_value = Some(value),
                "assignment" => {
                    if let Some(fn_node) = ["method_declaration", "constructor_declaration"]
                        .iter()
                        .find_map(|kind| get_enclosing_node_by_kind(capture.node, kind))
                    {
                        if let (Some(name), Some(params)) = (
                            get_field_string_from_node(fn_node, "name", code),
                            get_field_string_from_node(fn_node, "parameters", code),
                        ) {
                            scope = Some(Scope::Function(name.clone() + &params));
                            function_name = Some(name);
                            params_string = Some(params);
                        }
                    } else if let Some(class_node) =
                        get_enclosing_node_by_kind(capture.node, "class_declaration")
                    {
                        if let Some(name) = get_field_string_from_node(class_node, "name", code) {
                            scope = Some(Scope::Class(name));
                        }
                    } else {
                        scope = Some(Scope::Global);
                    }
                }
                _ => {}
            }

            let new_assignment = Assignment {
                variable_name: variable_name.clone().unwrap_or_default(),
                variable_type: variable_type.clone().unwrap_or_default(),
                value: variable_value.clone().unwrap_or_default(),
            };

            let assignment_key = AssignmentKey {
                scope: scope.clone().unwrap_or(Scope::Global),
                variable_name: variable_name.clone().unwrap_or_default(),
            };

            assignments_map.insert(assignment_key, new_assignment);
        });
        if let (Some(name), Some(params)) = (function_name, params_string) {
            let functions_params_assignments =
                create_assignments_from_function_params(&params, &name);
            assignments_map.extend(functions_params_assignments);
        }
    });
    assignments_map
}

fn create_assignments_from_function_params(
    function_params: &str,
    function_name: &str,
) -> Vec<(AssignmentKey, Assignment)> {
    let scope = Scope::Function(function_name.to_string() + function_params);
    let mut assignments = Vec::new();
    parse_callable_params(function_params)
        .iter()
        .for_each(|param| {
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
