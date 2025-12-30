use models::{Argument, Assignment, AssignmentKey, Scope};
use std::collections::HashMap;

use crate::parsing::parameters::parse_callable_params;

/// Inferring starts by trying to approximate value's type.
/// Then we look at assignments within the same function.
/// Then we look at assignments within the same class (entity).
/// Then we look at global assignments.
pub(in crate::extraction) fn infer_argument_type(
    arg: &mut Argument,
    enclosing_function_name: &Option<String>,
    enclosing_class_name: &Option<String>,
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) {
    if let Some(approximated_datatype) = approximate_type_for_value(&arg.value) {
        arg.datatype = approximated_datatype;
        return;
    }

    let function_scope = Scope::from_enclosing_function(enclosing_function_name.clone());
    if let Some(found_type) = resolve_type(&arg.value, &function_scope, assignments_map) {
        arg.datatype = found_type;
        return;
    }

    let class_scope = Scope::from_enclosing_class(enclosing_class_name.clone());
    if let Some(found_type) = resolve_type(&arg.value, &class_scope, assignments_map) {
        arg.datatype = found_type;
        return;
    }

    if let Some(found_type) = resolve_type(&arg.value, &Scope::Global, assignments_map) {
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

fn approximate_type_for_value(value: &str) -> Option<String> {
    classify_numeric(value).or_else(|| classify_string(value))
}

fn classify_numeric(value: &str) -> Option<String> {
    value
        .parse::<i64>()
        .map(|_| "int".to_string())
        .or_else(|_| value.parse::<f64>().map(|_| "double".to_string()))
        .ok()
}

fn classify_string(value: &str) -> Option<String> {
    let v = value.trim();

    // String literal
    if v.starts_with('"') && v.ends_with('"') {
        return Some("String".to_string());
    }

    // String concatenation
    if v.contains('+') && v.contains('"') {
        return Some("String".to_string());
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
    // and are either inside the same function or inside the same class (entity).
    let assignments = assignments_map
        .iter()
        .filter(|(key, _)| {
            key.variable_name == invoked_object
                && (Scope::from_enclosing_function(enclosing_function_name.clone()) == key.scope
                    || Scope::from_enclosing_class(enclosing_class_name.clone()) == key.scope)
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
            Scope::Function(func) => parse_callable_params(func)
                .into_iter()
                .find(|param| param.name == assignment.value)
                .and_then(|param| param.datatype),
            Scope::Global => None,
            Scope::Class(_) => None,
        })
        .next()
}
