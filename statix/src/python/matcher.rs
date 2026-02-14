use std::collections::HashMap;

use crate::{ast::CallableAst, matcher::CallableMatcher, symbolic::VarType};

#[derive(Clone, Default)]
pub struct PythonCallableMatcher {}

impl PythonCallableMatcher {
    pub fn new() -> Self {
        Self {}
    }
}

impl CallableMatcher for PythonCallableMatcher {
    fn find_closest_callable(
        &self,
        callables: &HashMap<String, CallableAst>,
        name: &str,
        params: &[VarType],
    ) -> Option<String> {
        let mut highest: usize = 0;
        let mut winner: Option<String> = None;
        let mangled_name = mangle_header(name);
        for header in callables.keys() {
            // Prefer match by using name mangling
            if let (Some(m_our), Some(m_to_cmp)) = (&mangled_name, &mangle_header(header))
                && m_our == m_to_cmp
            {
                return Some(header.clone());
            }
            // Fallback to match by highest matched params and matching name
            if let Some((cur_name, cur_params)) = parse_callable_header_manual(header) {
                if cur_name != name {
                    continue;
                }
                let matched = params
                    .iter()
                    .zip(cur_params.iter())
                    .filter(|(a, b)| a == b)
                    .count();
                if matched > highest {
                    highest = matched;
                    winner = Some(header.clone());
                }
            }
        }

        winner
    }

    fn clone_box(&self) -> Box<dyn CallableMatcher> {
        Box::new(self.clone())
    }
}

fn mangle_header(header: &str) -> Option<String> {
    if let Some((name, params)) = parse_callable_header_manual(header) {
        return Some(name + "(" + &params.join(",") + ")");
    }

    None
}

fn parse_callable_header_manual(header: &str) -> Option<(String, Vec<VarType>)> {
    let open_paren = header.find('(')?;
    let close_paren = header.rfind(')')?;

    let before_paren = &header[..open_paren].trim();
    let name = before_paren.split_whitespace().last()?.to_string();

    let params_content = &header[open_paren + 1..close_paren];

    let mut params = Vec::new();
    let mut current_param = String::new();
    let mut bracket_depth = 0;

    for c in params_content.chars() {
        match c {
            '<' => {
                bracket_depth += 1;
                current_param.push(c);
            }
            '>' => {
                bracket_depth -= 1;
                current_param.push(c);
            }
            ',' if bracket_depth == 0 => {
                let p = current_param.trim();
                if !p.is_empty() {
                    params.push(p.to_string());
                }
                current_param.clear();
            }
            _ => current_param.push(c),
        }
    }

    let last_p = current_param.trim();
    if !last_p.is_empty() {
        params.push(last_p.to_string());
    }

    Some((name, params))
}

/// Full header in Java is meant by the whole method header without accessibility specifier and parameters names
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
