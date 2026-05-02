use std::collections::HashMap;

use models::ParsedCallable;

use crate::{matcher::find_closest_callable_impl, symbolic::VarType};

#[derive(Clone, Default)]
pub struct JavaCallableMatcher {}

impl JavaCallableMatcher {
    pub fn new() -> Self {
        Self {}
    }
}

impl crate::matcher::CallableMatcher for JavaCallableMatcher {
    fn find_closest_callable(
        &self,
        callables: &HashMap<String, ParsedCallable>,
        name: &str,
        params: &[VarType],
    ) -> Option<String> {
        find_closest_callable_impl(callables, name, params)
    }

    fn clone_box(&self) -> Box<dyn crate::matcher::CallableMatcher> {
        Box::new(self.clone())
    }
}

/// Full header in Java is meant by the whole method header without accessibility specifier and parameters names
pub fn java_convert_full_header_to_mangled_name(header: &str) -> String {
    let open_paren = header.find('(').expect("Invalid method header");
    let close_paren = header.rfind(')').expect("Invalid method header");

    let prefix = header[..open_paren].trim();
    let params = header[open_paren + 1..close_paren].trim();

    let mangled_params = if params.is_empty() {
        String::new()
    } else {
        split_params(params)
            .into_iter()
            .map(|p| extract_param_type(&p))
            .collect::<Vec<_>>()
            .join(",")
    };

    format!("{}({})", prefix, mangled_params)
}

fn split_params(params: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut start = 0;

    for (i, c) in params.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                result.push(params[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }

    result.push(params[start..].trim().to_string());
    result
}

fn extract_param_type(param: &str) -> String {
    // Remove annotations and 'final'
    let tokens: Vec<&str> = param
        .split_whitespace()
        .filter(|t| !t.starts_with('@') && *t != "final")
        .collect();

    // Last token is parameter name
    tokens[..tokens.len() - 1].join(" ")
}
