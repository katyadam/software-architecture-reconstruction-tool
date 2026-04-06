use crate::extraction::calls::{
    PythonCallStatement,
    type_inference::{find_invoked_type, infer_argument_type},
};
use models::{Assignment, AssignmentKey, CallStatement, Scope};
use std::collections::HashMap;

/// Pass 2 variant: operates on language-agnostic `CallStatement`s.
/// Mirrors `evaluate_invocations` but without the `PythonCallStatement` wrapper.
pub fn evaluate_invocations_on_statements(
    calls: &mut [CallStatement],
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) {
    for call in calls {
        if let Some(pos) = call.function_name.rfind('.') {
            let invoked_object = &call.function_name[..pos];
            call.invoked_on = find_invoked_type(
                invoked_object,
                &call.enclosing_function_name,
                &call.enclosing_class_name,
                assignments_map,
            );
        }

        let scope = match &call.enclosing_function_name {
            Some(fname) => Scope::Function(fname.clone()),
            None => Scope::Global,
        };

        for arg in &mut call.arguments {
            infer_argument_type(arg, &scope, assignments_map);
        }
    }
}

/// Evaluates invocations and determines on what type each function is called.
#[deprecated]
pub fn evaluate_invocations(
    calls: &mut [PythonCallStatement],
    assignments_map: &HashMap<AssignmentKey, Assignment>,
) {
    for call in calls {
        if let Some(pos) = call.call_statement.function_name.rfind('.') {
            let invoked_object = &call.call_statement.function_name[..pos];
            call.call_statement.invoked_on = find_invoked_type(
                invoked_object,
                &call.call_statement.enclosing_function_name,
                &call.call_statement.enclosing_class_name,
                assignments_map,
            );
        }
        let scope = match &call.call_statement.enclosing_function_name {
            Some(fname) => Scope::Function(fname.clone()),
            None => Scope::Global,
        };

        for arg in &mut call.call_statement.arguments {
            infer_argument_type(arg, &scope, assignments_map);
        }
    }
}
