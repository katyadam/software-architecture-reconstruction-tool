use models::Field;
use tree_sitter::Node;

pub fn parse_superclasses(superclasses_node: Node, code: &str) -> Vec<String> {
    let cursor = &mut superclasses_node.walk();
    superclasses_node
        .named_children(cursor)
        .filter_map(|param| param.utf8_text(code.as_bytes()).ok().map(|s| s.to_string()))
        .collect()
}

pub fn parse_fields(fields_string: &str) -> Vec<Field> {
    // First strip () from the string
    let working_str = fields_string
        .strip_prefix('(')
        .unwrap_or(fields_string)
        .strip_suffix(')')
        .unwrap_or(fields_string);
    // For each parameter, extract its Field and collect only those that are Some()
    working_str
        .split([',', '\n'])
        .map(|s| s.trim_start())
        .filter(|s| *s != "self")
        .filter_map(parse_field)
        .collect()
}

/// Parses a single `__init__` parameter string into a [`Field`].
///
/// Handles four forms by splitting on `:` and `=`:
/// - `name` — bare name only
/// - `name: type` — typed field
/// - `name = value` — field with default value (no type annotation)
/// - `name: type = value` — typed field with default value
pub fn parse_field(field_string: &str) -> Option<Field> {
    let field_split: Vec<&str> = field_string.split([':', '=']).collect();
    match field_split.len() {
        0 => None,
        1 => Some(Field {
            name: field_split[0].trim().to_string(),
            datatype: None,
            initial_value: None,
            datatype_signature: None,
            is_collection: false,
        }),
        2 => {
            if field_string.contains(":") {
                let datatype = field_split[1].trim().to_string();
                let is_collection = is_collection_type(&datatype);
                Some(Field {
                    name: field_split[0].trim().to_string(),
                    datatype: Some(datatype),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection,
                })
            } else if field_string.contains("=") {
                Some(Field {
                    name: field_split[0].trim().to_string(),
                    datatype: None,
                    initial_value: Some(field_split[1].trim().to_string()),
                    datatype_signature: None,
                    is_collection: false,
                })
            } else {
                None
            }
        }
        3 => {
            let datatype = field_split[1].trim().to_string();
            let is_collection = is_collection_type(&datatype);
            Some(Field {
                name: field_split[0].trim().to_string(),
                datatype: Some(datatype),
                initial_value: Some(field_split[2].trim().to_string()),
                datatype_signature: None,
                is_collection,
            })
        }
        _ => None,
    }
}

/// Returns `true` if `datatype` represents a Python collection type.
///
/// Matches both the bare name (`list`, `List`) and the generic form (`list[int]`,
/// `Dict[str, int]`). Both lowercase (`list`, `dict`) and capitalised (`List`, `Dict`)
/// variants are recognised.
pub fn is_collection_type(datatype: &str) -> bool {
    let d = datatype.trim();
    if d.is_empty() {
        return false;
    }

    // Define the valid "roots" for collection types
    let collection_keywords = [
        "list", "dict", "set", "tuple", "deque", "sequence", "mapping", "iterable", "List", "Dict",
        "Set", "Tuple", "Deque", "Sequence", "Mapping", "Iterable",
    ];

    // Check if it's a simple keyword match: "list"
    if collection_keywords.contains(&d) {
        return true;
    }

    // Check if it's a generic match: "list[int]"
    // We ensure it starts with a keyword AND contains the opening bracket
    for key in collection_keywords {
        if d.starts_with(key) && d.contains('[') && d.ends_with(']') {
            return true;
        }
    }

    false
}
