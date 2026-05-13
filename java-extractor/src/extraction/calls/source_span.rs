use models::source_code::SourceSpan;
use tree_sitter::Node;

pub fn last_class_first_function_span(call_node: &Node, code: &str) -> SourceSpan {
    let mut method_node: Option<Node> = None;
    let mut class_node: Option<Node> = None;
    let mut node = *call_node;

    while let Some(parent) = node.parent() {
        match parent.kind() {
            "method_declaration" | "constructor_declaration" if method_node.is_none() => {
                method_node = Some(parent)
            }
            "class_declaration"
            | "interface_declaration"
            | "record_declaration"
            | "enum_declaration" => class_node = Some(parent),
            _ => {}
        }

        node = parent;
    }

    if let Some(class_node) = class_node {
        SourceSpan::new(class_node.start_byte() as u32, class_node.end_byte() as u32)
    } else if let Some(method_node) = method_node {
        SourceSpan::new(
            method_node.start_byte() as u32,
            method_node.end_byte() as u32,
        )
    } else {
        SourceSpan::new(0, code.as_bytes().len() as u32)
    }
}
