use crate::extraction::callables::parser::parse_parameters;
use models::{Argument, Assignment, AssignmentKey, Parameter, Scope};
use std::collections::HashMap;

pub(in crate::extraction) fn infer_argument_type(
    arg: &mut Argument,
    arg_enclosing_scope: &Scope,
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) {
    // Try resolving in local scope
    if let Some(found_type) = resolve_type(&arg.value, arg_enclosing_scope, assignments_map) {
        arg.datatype = found_type;
        return;
    }

    // Try resolving in global scope if local didn't find anything
    if arg.datatype == "any"
        && matches!(arg_enclosing_scope, Scope::Function(_))
        && let Some(found_type) = resolve_type(&arg.value, &Scope::Global, assignments_map)
    {
        arg.datatype = found_type;
    }
}

fn resolve_type(
    start_value: &str,
    scope: &Scope,
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) -> Option<String> {
    let mut current = start_value.to_string();

    while let Some(assignment) = assignments_map.get(&AssignmentKey {
        scope: scope.clone(),
        variable_name: current.clone(),
    }) {
        if !assignment.variable_type.is_empty() {
            return Some(assignment.variable_type.clone());
        }
        current = assignment.value.clone();
    }
    None
}

/// Finds the data type of an object used in a function invocation.
pub(in crate::extraction) fn find_invoked_type(
    invoked_object: &str,
    enclosing_function_name: &Option<String>,
    enclosing_class_name: &Option<String>,
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) -> Option<String> {
    // Find assignments that assign to invoked object variable
    // Take only those, where the variable_name in assignment is the invoked_object
    // and are either inside the same function or in constructor
    let assignments = assignments_map
        .iter()
        .filter(|(key, _)| {
            key.variable_name == invoked_object
                && (Scope::from_enclosings(
                    enclosing_function_name.clone(),
                    enclosing_class_name.clone(),
                ) == key.scope
                    || can_access_constructor(invoked_object, key))
        })
        .collect::<Vec<(&AssignmentKey, &Assignment)>>();

    // Prefer explicit assignment variable type first
    if let Some((_, assignment)) = assignments
        .clone()
        .iter()
        .find(|(_, val)| !val.variable_type.is_empty())
    {
        return Some(assignment.variable_type.clone());
    }

    // Otherwise, try to match a function parameter type
    assignments
        .iter()
        .filter_map(|(key, assignment)| match &key.scope {
            Scope::Function(func) => get_function_params(func)
                .into_iter()
                .find(|param| param.name == assignment.value)
                .and_then(|param| param.datatype),
            Scope::Global => None,
            Scope::Class(_) => None,
        })
        .next()
}

/// Extracts parameters from a function declaration string like `foo(bar, baz)`.
fn get_function_params(function_decl: &str) -> Vec<Parameter> {
    let params_str = function_decl
        .find('(')
        .zip(function_decl.rfind(')'))
        .map(|(start, end)| &function_decl[start..=end])
        .unwrap_or_default();

    parse_parameters(params_str)
}

fn can_access_constructor(invoked_object: &str, assignment_key: &AssignmentKey) -> bool {
    if let Scope::Function(fn_header) = &assignment_key.scope
        && invoked_object.starts_with("self.")
        && fn_header.starts_with("__init__(")
    {
        return true;
    }
    false
}
