use models::Parameter;

/// Parses a Java parameter list string (with or without outer parentheses) into
/// typed [`Parameter`] entries.
///
/// Each token is expected to be of the form `[@Annotation]* type name [= default]`.
/// Annotations (tokens starting with `@`) and the `final` modifier are stripped.
/// Parameters with fewer than two remaining tokens (i.e. no discernible type+name) are skipped.
pub fn parse_callable_params(params_string: &str) -> Vec<Parameter> {
    let params_string = strip_outer_parentheses(params_string);

    split_top_level_commas(params_string)
        .into_iter()
        .filter_map(|param| parse_single_param(&param))
        .collect()
}
fn parse_single_param(param: &str) -> Option<Parameter> {
    let param = param.trim();
    if param.is_empty() {
        return None;
    }

    // Split default value
    let (before_default, initial_value) = match param.split_once('=') {
        Some((left, right)) => (left.trim(), Some(right.trim().to_string())),
        None => (param, None),
    };

    // Remove annotations (@Something)
    let without_annotations = before_default
        .split_whitespace()
        .filter(|tok| !tok.starts_with('@'))
        .collect::<Vec<_>>();

    if without_annotations.len() < 2 {
        return None;
    }

    // Last token is parameter name
    let name = without_annotations.last()?.to_string();

    // Everything before is the type (including generics)
    let datatype = without_annotations[..without_annotations.len() - 1].join(" ");

    Some(Parameter {
        name,
        datatype: Some(datatype),
        initial_value,
    })
}

fn strip_outer_parentheses(input: &str) -> &str {
    let s = input.trim();
    if s.starts_with('(') && s.ends_with(')') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Splits `input` on `,` while ignoring commas that appear inside `<>` angle brackets.
/// This is necessary for correctly tokenizing generic parameter types such as
/// `Map<String, Integer>`, which must not be split at the inner comma.
fn split_top_level_commas(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut current = String::new();

    for c in input.chars() {
        match c {
            '<' => {
                depth += 1;
                current.push(c);
            }
            '>' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                result.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
    }

    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }

    result
}

pub fn normalize_whitespace(input: &str) -> String {
    input
        .replace(['\n', '\r'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
