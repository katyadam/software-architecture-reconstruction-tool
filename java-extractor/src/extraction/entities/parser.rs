use models::Field;
use tree_sitter::Node;

pub fn parse_formal_parameters(params_node: Node, code: &str) -> Vec<Field> {
    let mut fields = Vec::new();

    for i in 0..params_node.named_child_count() {
        if let Some(child) = params_node.named_child(i)
            && child.kind() == "formal_parameter"
            && let Some(field) = parse_formal_parameter(child, code)
        {
            fields.push(field);
        }
    }

    fields
}

fn parse_formal_parameter(node: Node, code: &str) -> Option<Field> {
    let mut name = String::new();
    let mut datatype = None;
    let mut is_collection = false;
    if let Some(datatype_node) = node.child_by_field_name("type") {
        let text = &code[datatype_node.start_byte()..datatype_node.end_byte()];
        is_collection = is_java_collection_type(text);
        datatype = Some(text.to_string());
    }

    if let Some(name_node) = node.child_by_field_name("name") {
        let text = &code[name_node.start_byte()..name_node.end_byte()];
        name = text.to_string();
    }

    Some(Field {
        name,
        datatype,
        initial_value: None,
        datatype_signature: None,
        is_collection,
    })
}

pub fn parse_field_declarations(fields_node: Node, code: &str) -> Vec<Field> {
    let mut fields = Vec::new();

    for i in 0..fields_node.named_child_count() {
        if let Some(child) = fields_node.named_child(i)
            && child.kind() == "field_declaration"
            && let Some(field) = parse_field_declaration(child, code)
        {
            fields.push(field);
        }
    }

    fields
}

fn parse_field_declaration(node: Node, code: &str) -> Option<Field> {
    let mut name = String::new();
    let mut datatype = None;
    let mut initial_value = None;
    let mut is_collection = false;
    if let Some(datatype_node) = node.child_by_field_name("type") {
        let text = &code[datatype_node.start_byte()..datatype_node.end_byte()];
        is_collection = is_java_collection_type(text);
        datatype = Some(text.to_string());
    }

    if let Some(decl_node) = node.child_by_field_name("declarator") {
        // name
        if let Some(name_node) = decl_node.child_by_field_name("name") {
            let text = &code[name_node.start_byte()..name_node.end_byte()];
            name = text.to_string();
        }

        if let Some(value_node) = decl_node.child_by_field_name("value") {
            let text = &code[value_node.start_byte()..value_node.end_byte()];
            initial_value = Some(text.to_string());
        }
    }

    if name.is_empty() {
        None
    } else {
        Some(Field {
            name,
            datatype,
            initial_value,
            datatype_signature: None,
            is_collection,
        })
    }
}

pub fn is_java_collection_type(datatype: &str) -> bool {
    let d = datatype.trim();
    if d.is_empty() {
        return false;
    }

    // 1. Arrays (e.g., String[], int[][]) or Varargs (String...)
    if d.ends_with(']') || d.ends_with("...") {
        return true;
    }

    // List of known Java collection base names
    let collection_keywords = [
        "List",
        "ArrayList",
        "LinkedList",
        "Vector",
        "Set",
        "HashSet",
        "TreeSet",
        "LinkedHashSet",
        "Map",
        "HashMap",
        "TreeMap",
        "Hashtable",
        "LinkedHashMap",
        "Queue",
        "Deque",
        "PriorityQueue",
        "Collection",
        "Iterable",
        "Stack",
    ];

    // 2. Extract the "Base Type"
    // For "List<String>", base is "List"
    // For "HashMap<K, V>", base is "HashMap"
    // For "Set", base is "Set"
    let base_type = d.split('<').next().unwrap_or("").trim();

    // 3. Check if the base type is in our whitelist, handle fully qualified names
    collection_keywords
        .iter()
        .any(|&k| base_type == k || base_type.ends_with(&format!(".{}", k)))
}
