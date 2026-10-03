use super::git_diff::{ImpactError, find_module_root};
use models::{ChangedElement, ChangedElementKind};
use std::{ops::RangeInclusive, path::Path};
use tree_sitter::{Node, Parser};

/// Maps one candidate Java diff range to its smallest safe source owner.
pub(super) fn map_java_range(
    project_root: &Path,
    source_path: &str,
    range: RangeInclusive<usize>,
) -> Result<Vec<ChangedElement>, ImpactError> {
    let code = std::fs::read_to_string(project_root.join(source_path))
        .map_err(|error| ImpactError::Source(error.to_string()))?;
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .expect("Java grammar is valid");
    let tree = parser
        .parse(&code, None)
        .expect("Java source parses into a tree");
    let module_root = find_module_root(project_root, source_path)
        .to_string_lossy()
        .into_owned();
    Ok(range
        .filter_map(|line| {
            map_java_line(tree.root_node(), line - 1, &module_root, source_path, &code)
        })
        .fold(Vec::new(), |mut elements, element| {
            if !elements.contains(&element) {
                elements.push(element);
            }
            elements
        }))
}

/// Maps one changed candidate line to its smallest declaration owner.
fn map_java_line(
    root: Node<'_>,
    line: usize,
    module_root: &str,
    source_path: &str,
    code: &str,
) -> Option<ChangedElement> {
    let owner = smallest_owner(root, line)?;
    Some(ChangedElement {
        module_root: module_root.to_owned(),
        source_path: source_path.to_owned(),
        kind: if owner.kind() == "method_declaration" {
            ChangedElementKind::Callable
        } else {
            ChangedElementKind::Class
        },
        callable_signature: if owner.kind() == "method_declaration" {
            method_signature(owner, code)
        } else {
            class_name(owner, code)
        },
    })
}

/// Finds the smallest method or class declaration containing a zero-based source line.
fn smallest_owner(node: Node<'_>, line: usize) -> Option<Node<'_>> {
    if line < node.start_position().row || line > node.end_position().row {
        return None;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(owner) = smallest_owner(child, line) {
            return Some(owner);
        }
    }
    matches!(
        node.kind(),
        "method_declaration" | "class_declaration" | "interface_declaration"
    )
    .then_some(node)
}

/// Returns a stable simple callable identity for later conservative matching.
fn method_signature(node: Node<'_>, code: &str) -> Option<String> {
    let class = enclosing_class(node, code)?;
    let method = node
        .child_by_field_name("name")?
        .utf8_text(code.as_bytes())
        .ok()?;
    Some(format!("{class}.{method}()"))
}

/// Returns the enclosing class name for a method declaration.
fn enclosing_class(node: Node<'_>, code: &str) -> Option<String> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(parent.kind(), "class_declaration" | "interface_declaration") {
            return class_name(parent, code);
        }
        current = parent.parent();
    }
    None
}

/// Returns a declaration's simple class name for class-level fallback matching.
fn class_name(node: Node<'_>, code: &str) -> Option<String> {
    node.child_by_field_name("name")?
        .utf8_text(code.as_bytes())
        .ok()
        .map(str::to_owned)
}
