use super::{ImpactError, ModuleCatalog};
use models::{ChangedElement, ChangedElementKind};
use std::{ops::RangeInclusive, path::Path};
use tree_sitter::{Node, Parser};

/// Maps one candidate Java diff range to its smallest safe source owner.
pub(super) fn map_java_range(
    project_root: &Path,
    catalog: &ModuleCatalog,
    source_path: &str,
    range: RangeInclusive<usize>,
) -> Result<Vec<ChangedElement>, ImpactError> {
    if !catalog.can_narrow_source(source_path) {
        return Ok(Vec::new());
    }
    let code = std::fs::read_to_string(project_root.join(source_path))
        .map_err(|error| ImpactError::Source(error.to_string()))?;
    let module_root = catalog
        .owner_for(source_path)
        .to_string_lossy()
        .into_owned();
    Ok(map_java_range_in_source(&code, &module_root, source_path, range))
}

/// Maps a line range in already-loaded Java source to changed elements.
///
/// Used for both candidate-side (new) and baseline-side (deleted) hunks so that
/// deletions can be attributed to a callable rather than falling back to module level.
pub(super) fn map_java_range_in_source(
    code: &str,
    module_root: &str,
    source_path: &str,
    range: RangeInclusive<usize>,
) -> Vec<ChangedElement> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .expect("Java grammar is valid");
    let tree = parser
        .parse(code, None)
        .expect("Java source parses into a tree");
    range
        .filter_map(|line| {
            map_java_line(tree.root_node(), line - 1, module_root, source_path, code)
        })
        .fold(Vec::new(), |mut elements, element| {
            if !elements.contains(&element) {
                elements.push(element);
            }
            elements
        })
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
        kind: match owner.kind() {
            "method_declaration" | "constructor_declaration" => ChangedElementKind::Callable,
            _ => ChangedElementKind::Class,
        },
        callable_signature: match owner.kind() {
            "method_declaration" => method_signature(owner, code),
            "constructor_declaration" => constructor_signature(owner, code),
            _ => class_name(owner, code),
        },
    })
}

/// Finds the smallest method, constructor, or class declaration containing a zero-based source line.
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
        "method_declaration"
            | "constructor_declaration"
            | "class_declaration"
            | "interface_declaration"
    )
    .then_some(node)
}

/// Returns a stable callable identity including parameter types for overload disambiguation.
fn method_signature(node: Node<'_>, code: &str) -> Option<String> {
    let class = enclosing_class(node, code)?;
    let method = node
        .child_by_field_name("name")?
        .utf8_text(code.as_bytes())
        .ok()?;
    let params = parameter_types(node, code);
    Some(format!("{class}.{method}({params})"))
}

/// Returns a constructor identity including parameter types.
fn constructor_signature(node: Node<'_>, code: &str) -> Option<String> {
    let name = node
        .child_by_field_name("name")?
        .utf8_text(code.as_bytes())
        .ok()?;
    let params = parameter_types(node, code);
    Some(format!("{name}({params})"))
}

/// Extracts a comma-separated list of parameter type names from a method or constructor node.
fn parameter_types(node: Node<'_>, code: &str) -> String {
    let params_node = match node.child_by_field_name("parameters") {
        Some(p) => p,
        None => return String::new(),
    };
    let mut types = Vec::new();
    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        match child.kind() {
            "formal_parameter" => {
                if let Some(type_node) = child.child_by_field_name("type") {
                    if let Ok(t) = type_node.utf8_text(code.as_bytes()) {
                        types.push(t.to_owned());
                    }
                }
            }
            "spread_parameter" => {
                // spread_parameter has no named "type" field — take the first named child.
                let mut sc = child.walk();
                if let Some(type_node) = child.named_children(&mut sc).next() {
                    if let Ok(t) = type_node.utf8_text(code.as_bytes()) {
                        types.push(format!("{t}..."));
                    }
                }
            }
            _ => {}
        }
    }
    types.join(", ")
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

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    fn parse_java(code: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_java::LANGUAGE.into())
            .unwrap();
        parser.parse(code, None).unwrap()
    }

    fn first_node_of_kind<'t>(root: tree_sitter::Node<'t>, kind: &str) -> Option<tree_sitter::Node<'t>> {
        if root.kind() == kind {
            return Some(root);
        }
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if let Some(found) = first_node_of_kind(child, kind) {
                return Some(found);
            }
        }
        None
    }

    #[test]
    fn method_signature_no_params() {
        let code = "class Svc { void run() {} }";
        let tree = parse_java(code);
        let node = first_node_of_kind(tree.root_node(), "method_declaration").unwrap();
        assert_eq!(method_signature(node, code).unwrap(), "Svc.run()");
    }

    #[test]
    fn method_signature_with_params() {
        let code = "class Svc { int process(String name, int count) { return 0; } }";
        let tree = parse_java(code);
        let node = first_node_of_kind(tree.root_node(), "method_declaration").unwrap();
        assert_eq!(method_signature(node, code).unwrap(), "Svc.process(String, int)");
    }

    #[test]
    fn method_signature_varargs() {
        let code = "class Svc { void log(String fmt, Object... args) {} }";
        let tree = parse_java(code);
        let node = first_node_of_kind(tree.root_node(), "method_declaration").unwrap();
        assert_eq!(method_signature(node, code).unwrap(), "Svc.log(String, Object...)");
    }

    #[test]
    fn constructor_is_callable() {
        let code = "class Order { Order(String id, int qty) {} }";
        let tree = parse_java(code);
        let node = first_node_of_kind(tree.root_node(), "constructor_declaration").unwrap();
        // constructor_signature should use the constructor name, not an enclosing class
        assert_eq!(constructor_signature(node, code).unwrap(), "Order(String, int)");
        // smallest_owner must pick up constructor_declaration
        let line = node.start_position().row;
        let owner = smallest_owner(tree.root_node(), line).unwrap();
        assert_eq!(owner.kind(), "constructor_declaration");
    }

    #[test]
    fn map_java_line_constructor() {
        let code = "class Order {\n  Order(String id) {}\n}\n";
        let tree = parse_java(code);
        let result = map_java_line(tree.root_node(), 1, "mod", "Order.java", code);
        let elem = result.unwrap();
        assert_eq!(elem.kind, ChangedElementKind::Callable);
        assert_eq!(elem.callable_signature.unwrap(), "Order(String)");
    }

    #[test]
    fn map_java_line_overloads_are_distinct() {
        let code = "class Svc {\n  void run(String s) {}\n  void run(int i) {}\n}\n";
        let tree = parse_java(code);
        let sig1 = map_java_line(tree.root_node(), 1, "mod", "Svc.java", code)
            .unwrap()
            .callable_signature
            .unwrap();
        let sig2 = map_java_line(tree.root_node(), 2, "mod", "Svc.java", code)
            .unwrap()
            .callable_signature
            .unwrap();
        assert_ne!(sig1, sig2, "overloads must produce distinct signatures");
        assert_eq!(sig1, "Svc.run(String)");
        assert_eq!(sig2, "Svc.run(int)");
    }
}
