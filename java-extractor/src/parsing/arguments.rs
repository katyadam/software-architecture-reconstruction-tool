use models::Argument;
use tree_sitter::Node;

pub fn parse_call_arguments(args_node: Node, code: &str) -> Vec<Argument> {
    let mut args = Vec::new();
    let mut cursor = args_node.walk();
    for param in args_node.named_children(&mut cursor) {
        if let Ok(arg_string) = param.utf8_text(code.as_bytes()) {
            args.push(Argument {
                assigned_variable: "".to_string(),
                value: arg_string.to_string(),
                datatype: "any".to_string(),
            });
        }
    }

    args
}
