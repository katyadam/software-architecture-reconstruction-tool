use std::collections::HashMap;

use models::{Argument, Assignment, AssignmentKey, RestCall, Scope};
use regex::Regex;

fn get_assignment(
    function_name: &str,
    variable_name: &str,
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) -> Option<Assignment> {
    let assignment_func = assignments_map.get(&AssignmentKey {
        scope: Scope::Function(function_name.to_owned()),
        variable_name: variable_name.to_owned(),
    });
    let assignment_global = assignments_map.get(&AssignmentKey {
        scope: Scope::Global,
        variable_name: variable_name.to_owned(),
    });

    match (assignment_func, assignment_global) {
        (Some(a1), Some(_)) => Some(a1.clone()),
        (Some(a1), None) => Some(a1.clone()),
        (None, Some(a2)) => Some(a2.clone()),
        (None, None) => None,
    }
}

pub fn evaluate_parameter(
    function_name: &str,
    param: &mut Argument,
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) {
    if let Some(assignment) = get_assignment(function_name, &param.value, assignments_map) {
        param.value = assignment.value.clone();
    }
}

pub fn evaluate_target_uri(
    function_name: &str,
    target_uri: &mut String,
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) {
    let re = Regex::new(r"\{([^}]+)\}").unwrap();

    let new_uri = re.replace_all(target_uri, |caps: &regex::Captures| {
        let variable_name = &caps[1];

        let assignment = match get_assignment(function_name, variable_name, assignments_map) {
            Some(a) => a,
            None => return caps[0].to_owned(), // keep placeholder
        };

        let value = assignment.value.as_str();

        // if empty => do not replace
        if value.is_empty() {
            return caps[0].to_owned();
        }

        // strip matching surrounding quotes only if they match as a pair
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            return value[1..value.len() - 1].to_owned();
        }

        value.to_owned()
    });

    *target_uri = new_uri.into_owned();
}

/// Assigns values to parameters or parts of target URI.
/// The values are taken from local or global assignments.
pub fn evaluate_local_and_global_assignments(
    restcalls: &mut [RestCall],
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) {
    restcalls.iter_mut().for_each(|rcall| {
        rcall.call_arguments.iter_mut().for_each(|param| {
            evaluate_parameter(&rcall.function_name, param, assignments_map);
        });

        evaluate_target_uri(&rcall.function_name, &mut rcall.target_uri, assignments_map);
    });
}
