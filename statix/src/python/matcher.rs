use std::collections::HashMap;

use models::ir::ast::CallableAst;

use crate::{matcher::find_closest_callable_impl, symbolic::VarType};

#[derive(Clone, Default)]
pub struct PythonCallableMatcher {}

impl PythonCallableMatcher {
    pub fn new() -> Self {
        Self {}
    }
}

impl crate::matcher::CallableMatcher for PythonCallableMatcher {
    fn find_closest_callable(
        &self,
        callables: &HashMap<String, CallableAst>,
        name: &str,
        params: &[VarType],
    ) -> Option<String> {
        find_closest_callable_impl(callables, name, params)
    }

    fn clone_box(&self) -> Box<dyn crate::matcher::CallableMatcher> {
        Box::new(self.clone())
    }
}

/// Full header in Python is meant by the whole function header without parameter names
pub fn python_convert_full_header_to_mangled_name(header: &str) -> String {
    // 1. Split name/params from return type
    // Input: "create_item(client, name: str, ...) -> str"
    let parts: Vec<&str> = header.split("->").collect();
    let main_part = parts[0].trim();
    let return_type = parts.get(1).map(|t| t.trim()).unwrap_or("Any");

    // 2. Locate parentheses for the name and parameters
    let open_paren = main_part.find('(').expect("Invalid method header");
    let close_paren = main_part.rfind(')').expect("Invalid method header");

    let prefix = main_part[..open_paren].trim();
    let params_str = main_part[open_paren + 1..close_paren].trim();

    // 3. Process parameters
    let mangled_params = if params_str.is_empty() {
        String::new()
    } else {
        split_python_params(params_str)
            .into_iter()
            .map(|p| extract_python_param_type(&p))
            .collect::<Vec<_>>()
            .join(",")
    };

    format!("{} {}({})", return_type, prefix, mangled_params)
}

fn split_python_params(params: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut depth = 0; // Tracks brackets [], {}, ()
    let mut start = 0;

    for (i, c) in params.char_indices() {
        match c {
            '[' | '{' | '(' => depth += 1,
            ']' | '}' | ')' => depth -= 1,
            ',' if depth == 0 => {
                result.push(params[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = params[start..].trim();
    if !last.is_empty() {
        result.push(last.to_string());
    }
    result
}

fn extract_python_param_type(param: &str) -> String {
    // Python params can be:
    // 1. "name: type"
    // 2. "name = default"
    // 3. "name: type = default"
    // 4. "name"

    // Split by ':' first to see if a type hint exists
    if let Some((_name, rest)) = param.split_once(':') {
        // Handle cases like "name: str = 'default'" -> take "str"
        let type_part = rest.split('=').next().unwrap().trim();
        return type_part.to_string();
    }

    // Special case for 'self' or 'cls' in methods (usually Any)
    if param == "self" || param == "cls" {
        return "Any".to_string();
    }

    // If no colon, it's untyped
    "Any".to_string()
}
