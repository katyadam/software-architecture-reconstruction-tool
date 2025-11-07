use std::collections::HashMap;

use models::{Argument, Assignment, AssignmentKey, RestCall};
use regex::Regex;

use crate::extraction::assignments::map::get_assignment;

pub fn evaluate_parameter(
    function_name: &String,
    param: &mut Argument,
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) {
    if let Some(assignment) = get_assignment(&function_name, &param.value, assignments_map) {
        param.value = assignment.value.clone();
    }
}

pub fn evaluate_target_uri(
    function_name: &String,
    target_uri: &mut String,
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) {
    let re = Regex::new(r"\{([^}]+)\}").unwrap();

    // Replace each placeholder in a single pass
    let new_uri = re.replace_all(&target_uri, |caps: &regex::Captures| {
        let variable_name = &caps[1]; // Variable name, that is in {} within the provided String
        if let Some(assignment) =
            get_assignment(function_name, &variable_name.to_string(), assignments_map)
        {
            if let Some(stripped) = assignment
                .value
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
            {
                stripped.to_string()
            } else {
                assignment.value.to_string()
            }
        } else {
            // Leave unchanged if no assignment found
            caps[0].to_string()
        }
    });

    *target_uri = new_uri.into_owned();
}

pub fn evaluate_restcalls(
    restcalls: &mut Vec<RestCall>,
    assignments_map: HashMap<AssignmentKey, Assignment>,
) {
    restcalls.iter_mut().for_each(|rcall| {
        rcall.call_arguments.iter_mut().for_each(|param| {
            evaluate_parameter(&rcall.function_name, param, &assignments_map);
        });

        evaluate_target_uri(
            &rcall.function_name,
            &mut rcall.target_uri,
            &assignments_map,
        );
    });
}
