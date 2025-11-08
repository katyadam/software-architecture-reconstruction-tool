use crate::extraction::callables::parser::parse_parameters;
use models::{Assignment, AssignmentKey, CallStatement, Parameter, Scope};
use std::collections::HashMap;

/// Evaluates invocations and determines on what type each function is called.
pub fn evaluate_invocations(
    calls: &mut [CallStatement],
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) {
    for call in calls {
        if let Some(pos) = call.function_name.rfind('.') {
            let invoked_object = &call.function_name[..pos];
            call.invoked_on = find_invoked_type(invoked_object, assignments_map);
        }
    }
}

/// Extracts parameters from a function declaration string like `foo(bar, baz)`.
fn get_function_params(function_decl: &str) -> Vec<Parameter> {
    let params_str = function_decl
        .find('(')
        .zip(function_decl.rfind(')'))
        .map(|(start, end)| &function_decl[start..=end])
        .unwrap_or_default();

    parse_parameters(&params_str.to_string())
}

/// Finds the data type of an object used in a function invocation.
fn find_invoked_type(
    invoked_object: &str,
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) -> Option<String> {
    // Find assignments that assign to invoked object variable
    let assignments = assignments_map
        .iter()
        .filter(|(key, _)| key.variable_name == invoked_object);

    // Prefer explicit assignment variable type first
    if let Some((_, assignment)) = assignments
        .clone()
        .find(|(_, val)| !val.variable_type.is_empty())
    {
        return Some(assignment.variable_type.clone());
    }

    // Otherwise, try to match a function parameter type
    assignments
        .filter_map(|(key, assignment)| match &key.scope {
            Scope::Function(func) => get_function_params(func)
                .into_iter()
                .find(|param| param.name == assignment.value)
                .and_then(|param| param.datatype),
            Scope::Global => None,
        })
        .next()
}
