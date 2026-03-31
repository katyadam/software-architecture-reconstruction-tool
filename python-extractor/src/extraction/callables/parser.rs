use models::Parameter;

fn split_top_level_commas(s: &str) -> Vec<String> {
    statix::strings::split_at_top_level(s, &[',', '\n'], &[('(', ')'), ('[', ']'), ('{', '}')])
}

/// Parse a function parameter list like "(a: int = 3, b: str = Path(..., gt=0))"
pub fn parse_parameters(parameters_string: &str) -> Vec<Parameter> {
    // Gracefully handle functions with no params: "()" or empty string
    let working_str = parameters_string
        .trim()
        .strip_prefix('(')
        .unwrap_or(parameters_string)
        .strip_suffix(')')
        .unwrap_or(parameters_string)
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
pub fn parse_field(parameter_string: &str) -> Option<Parameter> {
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
