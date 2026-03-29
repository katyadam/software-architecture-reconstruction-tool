use models::Field;
use tree_sitter::Node;

fn collect_children_by_kind<T>(
    node: Node,
    code: &str,
    kind: &str,
    parse_fn: impl Fn(Node, &str) -> Option<T>,
) -> Vec<T> {
    (0..node.named_child_count())
        .filter_map(|i| node.named_child(i))
        .filter(|child| child.kind() == kind)
        .filter_map(|child| parse_fn(child, code))
        .collect()
}

pub fn parse_formal_parameters(params_node: Node, code: &str) -> Vec<Field> {
    collect_children_by_kind(
        params_node,
        code,
        "formal_parameter",
        parse_formal_parameter,
    )
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
    collect_children_by_kind(
        fields_node,
        code,
        "field_declaration",
        parse_field_declaration,
    )
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

/// Returns `true` if `datatype` represents a Java collection:
/// - arrays (`String[]`, `int[][]`) or varargs (`String...`)
/// - well-known generic collection base types (`List`, `Map`, `Set`, `Queue`, etc.)
///   matched against the token before the first `<`, including fully-qualified names
///
/// Used to populate [`models::Field::is_collection`].
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
