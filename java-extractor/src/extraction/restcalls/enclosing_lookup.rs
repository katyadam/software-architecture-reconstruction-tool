use sha2::{Digest, Sha256};
use tree_sitter::Node;

pub fn get_enclosing_function_signature_and_hash(node: Node, code: &str) -> (String, String) {
    let mut curr = node;
    while let Some(parent) = curr.parent() {
        if parent.kind() == "method_declaration" {
            return (
                get_function_node_signature(parent, code),
                get_function_node_hash(parent, code),
            );
        }
        curr = parent;
    }

    ("".to_string(), "".to_string())
}

fn get_function_node_signature(node: Node, code: &str) -> String {
    if let (Some(name_node), Some(params_node)) = (
        node.child_by_field_name("name"),
        node.child_by_field_name("parameters"),
    ) {
        let name =
            String::from_utf8_lossy(&code.as_bytes()[name_node.start_byte()..name_node.end_byte()]);
        let params_string = String::from_utf8_lossy(
            &code.as_bytes()[params_node.start_byte()..params_node.end_byte()],
        );

        return format!("{name}{params_string}");
    }

    "".to_string()
}

fn get_function_node_hash(node: Node, code: &str) -> String {
    let function_string =
        String::from_utf8_lossy(&code.as_bytes()[node.start_byte()..node.end_byte()]);
    let mut hasher = Sha256::new();
    hasher.update(function_string.as_bytes());
    format!("{:x}", hasher.finalize())
}
