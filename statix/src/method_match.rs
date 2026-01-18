use std::collections::HashMap;

use crate::{ast::MethodAst, symbolic::VarType};

pub fn find_closest_method(
    methods: &HashMap<String, MethodAst>,
    name: &str,
    params: &[VarType],
) -> Option<String> {
    let mut highest: usize = 0;
    let mut winner: Option<String> = None;
    let mangled_name = mangle_header(name);
    for header in methods.keys() {
        // Prefer match by using name mangling
        if let (Some(m_our), Some(m_to_cmp)) = (&mangled_name, &mangle_header(header))
            && m_our == m_to_cmp
        {
            return Some(header.clone());
        }
        // Fallback to match by highest matched params and matching name
        if let Some((cur_name, cur_params)) = parse_method_header_manual(header) {
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

fn mangle_header(header: &str) -> Option<String> {
    if let Some((name, params)) = parse_method_header_manual(header) {
        return Some(name + "(" + &params.join(",") + ")");
    }

    None
}

fn parse_method_header_manual(header: &str) -> Option<(String, Vec<VarType>)> {
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
pub fn convert_full_header_to_mangled_name(header: &str) -> String {
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
