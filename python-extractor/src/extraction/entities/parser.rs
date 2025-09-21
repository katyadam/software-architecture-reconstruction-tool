use models::Field;
use tree_sitter::Node;

pub fn parse_superclasses(superclasses_node: Node, code: &str) -> Vec<String> {
    let cursor = &mut superclasses_node.walk();
    superclasses_node
        .named_children(cursor)
        .filter_map(|param| param.utf8_text(code.as_bytes()).ok().map(|s| s.to_string()))
        .collect()
}

pub fn parse_fields(fields_string: &String) -> Vec<Field> {
    // First strip () from the string
    let working_str = fields_string
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

pub fn parse_field(field_string: &String) -> Option<Field> {
    let field_split: Vec<&str> = field_string.split(|c| c == ':' || c == '=').collect();
    match field_split.len() {
        0 => None,
        1 => Some(Field {
            name: field_split[0].trim().to_string(),
            datatype: None,
            initial_value: None,
            datatype_signature: None,
        }),
        2 => {
            if field_string.contains(":") {
                Some(Field {
                    name: field_split[0].trim().to_string(),
                    datatype: Some(field_split[1].trim().to_string()),
                    initial_value: None,
                    datatype_signature: None,
                })
            } else if field_string.contains("=") {
                Some(Field {
                    name: field_split[0].trim().to_string(),
                    datatype: None,
                    initial_value: Some(field_split[1].trim().to_string()),
                    datatype_signature: None,
                })
            } else {
                None
            }
        }
        3 => Some(Field {
            name: field_split[0].trim().to_string(),
            datatype: Some(field_split[1].trim().to_string()),
            initial_value: Some(field_split[0].trim().to_string()),
            datatype_signature: None,
        }),
        _ => None,
    }
}
