use tree_sitter::Node;

pub fn get_enclosing_element_name(node: Node, code: &str) -> String {
    let mut curr = node;
    while let Some(parent) = curr.parent() {
        if parent.kind() == "class_declaration"
            || parent.kind() == "interface_declaration"
            || parent.kind() == "record_declaration"
            || parent.kind() == "enum_declaration"
        {
            if let Some(name_node) = parent.child_by_field_name("name") {
                let name = String::from_utf8_lossy(
                    &code.as_bytes()[name_node.start_byte()..name_node.end_byte()],
                );

                return name.to_string();
            }
        }
        curr = parent;
    }

    "".to_string()
}
