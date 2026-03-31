use std::collections::HashMap;

use crate::{ast::CallableAst, symbolic::VarType};

pub trait CallableMatcher {
    fn find_closest_callable(
        &self,
        callables: &HashMap<String, CallableAst>,
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
/// First tries an exact match via mangled name. Falls back to matching by
/// function name + highest number of matching parameter types.
/// Uses `winner.is_none() || matched > highest` so zero-param callees resolve correctly.
pub fn find_closest_callable_impl(
    callables: &HashMap<String, CallableAst>,
    name: &str,
    params: &[VarType],
) -> Option<String> {
    let mut highest: usize = 0;
    let mut winner: Option<String> = None;
    let mangled_name = name.to_owned() + "(" + &params.join(",") + ")";

    for header in callables.keys() {
        // Prefer exact match by mangled name
        if let Some(m_to_cmp) = mangle_header(header)
            && mangled_name == m_to_cmp
        {
            return Some(header.clone());
        }
        // Fallback: match by name + highest parameter type overlap
        if let Some((cur_name, cur_params)) = parse_callable_header_manual(header) {
            if cur_name != name {
                continue;
            }
            let matched = params
                .iter()
                .zip(cur_params.iter())
                .filter(|(a, b)| a == b)
                .count();
            if winner.is_none() || matched > highest {
                highest = matched;
                winner = Some(header.clone());
            }
        }
    }

    winner
}

/// Mangle a full callable header string into `name(Type1,Type2)` form,
/// stripping return type and parameter names.
pub fn mangle_header(header: &str) -> Option<String> {
    let (name, params) = parse_callable_header_manual(header)?;
    Some(name + "(" + &params.join(",") + ")")
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
