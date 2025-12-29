use sha2::{Digest, Sha256};
use tree_sitter::Node;

pub fn get_enclosing_node_by_kind<'tree>(
    node: Node<'tree>,
    kind: &'tree str,
) -> Option<Node<'tree>> {
    let mut curr = node;
    while let Some(parent) = curr.parent() {
        if parent.kind() == kind {
            return Some(parent);
        }
        curr = parent;
    }

    None
}

pub fn get_field_string_from_node<'tree>(
    node: Node<'tree>,
    field_name: &'tree str,
    code: &'tree str,
) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name(field_name) {
        return Some(
            String::from_utf8_lossy(&code.as_bytes()[name_node.start_byte()..name_node.end_byte()])
                .to_string(),
        );
    }
    None
}

pub fn get_hashed_node_value<'tree>(node: Node<'tree>, code: &'tree str) -> String {
    let mut hasher = Sha256::new();
    let value =
        String::from_utf8_lossy(&code.as_bytes()[node.start_byte()..node.end_byte()]).to_string();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}
