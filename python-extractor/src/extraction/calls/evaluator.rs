use std::collections::HashMap;

use models::{Assignment, AssignmentKey, CallStatement};

pub fn evaluate_invocations(
    calls: &mut Vec<CallStatement>,
    assignments_map: HashMap<AssignmentKey, Assignment>,
) {
    calls.into_iter().for_each(|call| {
        if let Some(pos) = call.function_name.rfind('.') {
            call.invoked_on = Some(call.function_name[..pos].to_string());
        }
    });
}
