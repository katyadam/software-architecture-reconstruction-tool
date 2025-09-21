use models::Argument;
use tree_sitter::Node;

pub fn extract_param_names(params_node: Node, code: &str) -> Vec<String> {
    let mut names = vec![];
    let mut cursor = params_node.walk();
    for param in params_node.named_children(&mut cursor) {
        if let Some(name_node) = param.child_by_field_name("name") {
            let name = name_node.utf8_text(code.as_bytes()).unwrap().to_string();
            names.push(name);
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
            });
        }
    }
    arguments
}

pub fn clean_formatted_python_string(string: String) -> String {
    string
        .strip_prefix("f\"")
        .unwrap()
        .strip_suffix("\"")
        .unwrap()
        .to_string()
}
