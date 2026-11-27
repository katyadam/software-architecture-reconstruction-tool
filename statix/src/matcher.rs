use std::collections::HashMap;

use models::ParsedCallable;

use crate::symbolic::VarType;

pub trait CallableMatcher {
    fn find_closest_callable(
        &self,
        callables: &HashMap<String, ParsedCallable>,
        name: &str,
        params: &[VarType],
    ) -> Option<String>;

    fn clone_box(&self) -> Box<dyn CallableMatcher>;
}

impl Clone for Box<dyn CallableMatcher> {
    fn clone(&self) -> Box<dyn CallableMatcher> {
        self.clone_box()
    }
}

/// Shared fallback matching logic used by both Java and Python matchers.
///
/// Picks the callable with the same name and arity that has the most matching
/// parameter types (an exact match matches them all). A candidate with zero
/// matching types still counts, so zero-param callees resolve correctly.
/// Ties go to the smallest key: `HashMap` order changes between runs (#62).
pub fn find_closest_callable_impl(
    callables: &HashMap<String, ParsedCallable>,
    name: &str,
    params: &[VarType],
) -> Option<String> {
    let mut highest: usize = 0;
    let mut winner: Option<&String> = None;

    for header in callables.keys() {
        let Some((cur_name, cur_params)) = parse_callable_header_manual(header) else {
            continue;
        };
        if cur_name != name || cur_params.len() != params.len() {
            continue;
        }
        let matched = params
            .iter()
            .zip(cur_params.iter())
            .filter(|(a, b)| a == b)
            .count();
        if winner.is_none_or(|w| matched > highest || (matched == highest && header < w)) {
            highest = matched;
            winner = Some(header);
        }
    }

    winner.cloned()
}

/// Parse a callable header string into `(name, [param_types])`.
/// Handles generic brackets `<>` to avoid splitting on commas inside type parameters.
pub fn parse_callable_header_manual(header: &str) -> Option<(String, Vec<VarType>)> {
    let open_paren = header.find('(')?;
    let close_paren = header.rfind(')')?;

    let before_paren = header[..open_paren].trim();
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
                    params.push(Some(p.to_string()));
                }
                current_param.clear();
            }
            _ => current_param.push(c),
        }
    }

    let last_p = current_param.trim();
    if !last_p.is_empty() {
        params.push(Some(last_p.to_string()));
    }

    Some((name, params))
}
