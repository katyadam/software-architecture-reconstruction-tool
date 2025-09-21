use models::Parameter;

// One can see that this code is the same as in entities.rs
// difference is that Field and Parameter have different meanings, even though they have same attributes.
pub fn parse_parameters(parameters_string: &String) -> Vec<Parameter> {
    // First strip () from the string
    let working_str = parameters_string
        .strip_prefix("(")
        .unwrap()
        .strip_suffix(")")
        .unwrap();
    // For each parameter, extract its Field and collect only those that are Some()
    working_str
        .split(|c| c == ',' || c == '\n')
        .map(|s| s.trim_start())
        .filter_map(|s| parse_field(&s.to_string()))
        .collect()
}

pub fn parse_field(parameter_string: &String) -> Option<Parameter> {
    let field_split: Vec<&str> = parameter_string.split(|c| c == ':' || c == '=').collect();
    match field_split.len() {
        0 => None,
        1 => Some(Parameter {
            name: field_split[0].trim().to_string(),
            datatype: None,
            initial_value: None,
        }),
        2 => {
            if parameter_string.contains(":") {
                Some(Parameter {
                    name: field_split[0].trim().to_string(),
                    datatype: Some(field_split[1].trim().to_string()),
                    initial_value: None,
                })
            } else if parameter_string.contains("=") {
                Some(Parameter {
                    name: field_split[0].trim().to_string(),
                    datatype: None,
                    initial_value: Some(field_split[1].trim().to_string()),
                })
            } else {
                None
            }
        }
        3 => Some(Parameter {
            name: field_split[0].trim().to_string(),
            datatype: Some(field_split[1].trim().to_string()),
            initial_value: Some(field_split[0].trim().to_string()),
        }),
        _ => None,
    }
}
