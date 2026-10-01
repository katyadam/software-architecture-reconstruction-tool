use std::collections::HashMap;

use models::{
    Scope,
    ir::project::{ConstantValue, TypedFileRecord},
};

/// Collect project-wide constants from global-scope assignments with literal values.
///
/// A constant is any `Scope::Global` assignment whose name is `UPPER_SNAKE_CASE`
/// and whose value is a string literal, numeric literal, or boolean.
/// First definition wins on name collisions (import order is non-deterministic).
pub fn collect_constants(files: &[TypedFileRecord]) -> HashMap<String, ConstantValue> {
    let mut constants = HashMap::new();
    for file in files {
        for (key, assignment) in &file.assignments {
            if matches!(key.scope, Scope::Global | Scope::Class(_))
                && is_constant_name(&key.variable_name)
            {
                constants
                    .entry(key.variable_name.clone())
                    .or_insert_with(|| ConstantValue {
                        name: key.variable_name.clone(),
                        value: assignment.value.clone(),
                        source_file: file.file_path.clone(),
                    });
            }
        }
    }
    constants
}

/// A constant name is non-empty and consists only of uppercase ASCII letters, digits, and `_`.
fn is_constant_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
}
