use std::path::Path;
use tree_sitter::{Node, Parser};

/// Finds an unambiguous JVM owner, including lexical nested classes, in source.
/// Ambiguous local/same-named declarations remain unresolved.
pub fn declaring_type_for_callable(path: &Path, class_name: &str) -> Option<String> {
    let source = std::fs::read_to_string(path).ok()?;
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .ok()?;
    let tree = parser.parse(&source, None)?;
    if tree.root_node().has_error() {
        return None;
    }
    let mut cursor = tree.root_node().walk();
    let package = tree
        .root_node()
        .named_children(&mut cursor)
        .find(|n| n.kind() == "package_declaration")
        .and_then(|n| {
            let mut cursor = n.walk();
            n.named_children(&mut cursor)
                .find(|n| matches!(n.kind(), "identifier" | "scoped_identifier"))
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .map(|s| s.replace('.', "/"))
        })
        .unwrap_or_default();
    fn visit(
        node: Node<'_>,
        source: &str,
        class: &str,
        stack: &mut Vec<String>,
        matches: &mut Vec<String>,
    ) {
        let is_type = matches!(
            node.kind(),
            "class_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "record_declaration"
                | "annotation_type_declaration"
        );
        if is_type {
            let Some(name) = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
            else {
                return;
            };
            stack.push(name.into());
            if name == class {
                matches.push(stack.join("$"));
            }
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            visit(child, source, class, stack, matches);
        }
        if is_type {
            stack.pop();
        }
    }
    let mut matches = Vec::new();
    visit(
        tree.root_node(),
        &source,
        class_name,
        &mut Vec::new(),
        &mut matches,
    );
    if matches.len() != 1 {
        return None;
    }
    Some(format!(
        "L{}{}{}",
        package,
        if package.is_empty() { "" } else { "/" },
        matches[0]
    ))
}
