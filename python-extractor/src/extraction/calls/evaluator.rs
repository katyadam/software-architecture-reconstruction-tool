use crate::extraction::calls::type_inference::{find_invoked_type, infer_argument_type};
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
