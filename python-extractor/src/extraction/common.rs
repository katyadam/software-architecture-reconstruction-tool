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
                datatype: None,
            });
        }
    }
    arguments
}

/// Returns the source text slice covered by `node`.
pub fn node_text<'a>(node: tree_sitter::Node, code: &'a str) -> &'a str {
    &code[node.start_byte()..node.end_byte()]
}

/// Walks the parent chain to find the nearest enclosing `function_definition`
/// and `class_definition` of `node`. Returns a 3-tuple of:
/// - the full function signature (`"name(params) -> return_type"`), or `None`
/// - the enclosing class name, or `None`
/// - the SHA-256 hex hash of the function body (for deduplication), or `None`
pub(crate) fn find_enclosing_information(
    mut node: Node,
    code: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let mut function_node: Option<Node> = None;
    let mut class_node: Option<Node> = None;

    while let Some(parent) = node.parent() {
        match parent.kind() {
            "function_definition" if function_node.is_none() => function_node = Some(parent),
            "class_definition" if class_node.is_none() => class_node = Some(parent),
            _ => {}
        }

        // Optimization: stop if we found both
        if function_node.is_some() && class_node.is_some() {
            break;
        }
        node = parent;
    }

    let mut function_name_full = None;
    let mut function_hash = None;

    if let Some(f_node) = function_node {
        let name = f_node
            .child_by_field_name("name")
            .map(|n| node_text(n, code))
            .unwrap_or("unknown");

        let params = f_node
            .child_by_field_name("parameters")
            .map(|n| node_text(n, code))
            .unwrap_or("()");

        let return_type = f_node
            .child_by_field_name("return_type")
            .map(|n| node_text(n, code))
            .unwrap_or("Any");

        function_name_full = Some(format!("{}{} -> {}", name, params, return_type));

        function_hash = Some(statix::strings::hash_text(node_text(f_node, code)));
    }

    // 3. Process Class Information
    let class_name = class_node
        .and_then(|n| n.child_by_field_name("name"))
        .map(|n| node_text(n, code).to_string());

    (function_name_full, class_name, function_hash)
}
