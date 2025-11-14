use models::Parameter;

/// Split a parameter list into top-level tokens.
/// Commas inside (), [], {} are ignored.
fn split_top_level_commas(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth_paren = 0i32;
    let mut depth_brack = 0i32;
    let mut depth_brace = 0i32;
    let mut current = String::new();

    for ch in s.chars() {
        match ch {
            '(' => {
                depth_paren += 1;
                current.push(ch);
            }
            ')' => {
                depth_paren = (depth_paren - 1).max(0);
                current.push(ch);
            }
            '[' => {
                depth_brack += 1;
                current.push(ch);
            }
            ']' => {
                depth_brack = (depth_brack - 1).max(0);
                current.push(ch);
            }
            '{' => {
                depth_brace += 1;
                current.push(ch);
            }
            '}' => {
                depth_brace = (depth_brace - 1).max(0);
                current.push(ch);
            }
            ',' => {
                // only split on commas at top level
                if depth_paren == 0 && depth_brack == 0 && depth_brace == 0 {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_string());
                    }
                    current.clear();
                } else {
                    current.push(ch);
                }
            }
            '\n' => {
                if depth_paren == 0 && depth_brack == 0 && depth_brace == 0 {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_string());
                    }
                    current.clear();
                } else {
                    current.push(ch);
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
}

/// Parse a function parameter list like "(a: int = 3, b: str = Path(..., gt=0))"
pub fn parse_parameters(parameters_string: &str) -> Vec<Parameter> {
    // Gracefully handle functions with no params: "()" or empty string
    let working_str = parameters_string
        .trim()
        .strip_prefix('(')
        .unwrap_or(parameters_string)
        .strip_suffix(')')
        .unwrap_or_else(|| parameters_string)
        .trim()
        .to_string();

    if working_str.is_empty() {
        return Vec::new();
    }

    split_top_level_commas(&working_str)
        .into_iter()
        .filter_map(|token| parse_field(&token))
        .collect()
}

/// Parse a single parameter expression into a `Parameter`.
pub fn parse_field(parameter_string: &String) -> Option<Parameter> {
    let s = parameter_string.trim();

    if s.is_empty() {
        return None;
    }

    // find first ':' and first '=' positions (if any)
    let colon_pos = s.find(':');
    let eq_pos = s.find('=');

    match (colon_pos, eq_pos) {
        (None, None) => {
            // bare parameter name
            Some(Parameter {
                name: s.to_string(),
                datatype: None,
                initial_value: None,
            })
        }
        (Some(colon), None) => {
            // "name: type"
            let name = s[..colon].trim().to_string();
            let dtype = s[colon + 1..].trim().to_string();
            Some(Parameter {
                name,
                datatype: Some(dtype),
                initial_value: None,
            })
        }
        (None, Some(eq)) => {
            // "name = default"
            let name = s[..eq].trim().to_string();
            let init = s[eq + 1..].trim().to_string();
            Some(Parameter {
                name,
                datatype: None,
                initial_value: Some(init),
            })
        }
        (Some(colon), Some(eq)) => {
            if colon < eq {
                // "name: type = default"
                let name = s[..colon].trim().to_string();
                let dtype = s[colon + 1..eq].trim().to_string();
                let init = s[eq + 1..].trim().to_string();
                Some(Parameter {
                    name,
                    datatype: Some(dtype),
                    initial_value: Some(init),
                })
            } else {
                // fallback, e.g. "name = something: weird"
                let name = s[..eq].trim().to_string();
                let init = s[eq + 1..].trim().to_string();
                Some(Parameter {
                    name,
                    datatype: None,
                    initial_value: Some(init),
                })
            }
        }
    }
}
