use models::source_code::SourceSpan;
use tree_sitter::Node;

pub fn last_class_last_function_span(call_node: &Node, code: &str) -> SourceSpan {
    let mut function_node: Option<Node> = None;
    let mut class_node: Option<Node> = None;
    let mut node = *call_node;

    while let Some(parent) = node.parent() {
        match parent.kind() {
            "function_definition" => function_node = Some(parent),
            "class_definition" => class_node = Some(parent),
            _ => {}
        }

        node = parent;
    }

    if let Some(class_node) = class_node {
        SourceSpan::new(class_node.start_byte() as u32, class_node.end_byte() as u32)
    } else if let Some(function_node) = function_node {
        SourceSpan::new(
            function_node.start_byte() as u32,
            function_node.end_byte() as u32,
        )
    } else {
        SourceSpan::new(0, code.as_bytes().len() as u32)
    }
}
