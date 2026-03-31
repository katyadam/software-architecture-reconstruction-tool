use models::Argument;
use tree_sitter::Node;

pub fn extract_param_names(params_node: Node, code: &str) -> Vec<String> {
    let mut names = vec![];
    let mut cursor = params_node.walk();

    for param in params_node.named_children(&mut cursor) {
        match param.kind() {
            "identifier" => {
                names.push(code[param.start_byte()..param.end_byte()].to_string());
            }
            "typed_parameter" => {
                if let Some(ident_node) =
                    param.child_by_field_name("name").or_else(|| param.child(0))
                {
                    names.push(code[ident_node.start_byte()..ident_node.end_byte()].to_string());
                }
            }
            _ => {}
        }
    }

    names
}

pub fn extract_function_arguments(function_node: Node, code: &str) -> Vec<Argument> {
    let mut arguments: Vec<Argument> = vec![];
    let mut cursor = function_node.walk();
    for param in function_node.named_children(&mut cursor) {
        let variable_name = param
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(code.as_bytes()).ok());

        let variable_value = param
            .child_by_field_name("value")
            .and_then(|n| n.utf8_text(code.as_bytes()).ok());

        if let (Some(name), Some(value)) = (variable_name, variable_value) {
            arguments.push(Argument {
                assigned_variable: name.to_string(),
                value: value.to_string(),
                datatype: "any".to_string(),
            });
        }
    }
    arguments
}

/// Returns the source text slice covered by `node`.
pub fn node_text<'a>(node: tree_sitter::Node, code: &'a str) -> &'a str {
    &code[node.start_byte()..node.end_byte()]
}
