use tree_sitter::Node;

/// Walks the parent chain of `node` upward and returns the first ancestor
/// whose [`Node::kind`] equals `kind`. Returns `None` if the root is reached
/// without a match.
pub fn get_enclosing_node_by_kind<'tree>(
    node: Node<'tree>,
    kind: &'tree str,
) -> Option<Node<'tree>> {
    let mut curr = node;
    while let Some(parent) = curr.parent() {
        if parent.kind() == kind {
            return Some(parent);
        }
        curr = parent;
    }

    None
}

/// Returns the source text of the named field child `field_name` of `node`,
/// or `None` if the field is absent.
pub fn get_field_string_from_node<'tree>(
    node: Node<'tree>,
    field_name: &'tree str,
    code: &'tree str,
) -> Option<String> {
    node.child_by_field_name(field_name)
        .map(|n| code[n.start_byte()..n.end_byte()].to_string())
}

/// Returns a SHA-256 hex digest of the source text spanning `node`.
/// Used to fingerprint code elements (e.g. callables) for deduplication across files.
pub fn get_hashed_node_value<'tree>(node: Node<'tree>, code: &'tree str) -> String {
    let value = &code[node.start_byte()..node.end_byte()];
    statix::strings::hash_text(value)
}
