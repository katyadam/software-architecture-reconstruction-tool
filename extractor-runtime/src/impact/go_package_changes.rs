//! Byte-level package-policy eligibility supplements the legacy line mapper.

pub(super) fn only_function_bodies_changed(old: &[u8], new: &[u8]) -> bool {
    fn outside_bodies(source: &[u8]) -> Option<Vec<Vec<u8>>> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_go::LANGUAGE.into()).ok()?;
        let tree = parser.parse(source, None)?;
        if tree.root_node().has_error() {
            return None;
        }
        let mut cursor = tree.root_node().walk();
        let mut offset = 0;
        let mut sections = vec![];
        for node in tree.root_node().named_children(&mut cursor) {
            if !matches!(node.kind(), "function_declaration" | "method_declaration") {
                continue;
            }
            if let Some(body) = node.child_by_field_name("body") {
                sections.push(source[offset..body.start_byte()].to_vec());
                offset = body.end_byte();
            }
        }
        sections.push(source[offset..].to_vec());
        Some(sections)
    }
    old != new && outside_bodies(old).is_some_and(|a| outside_bodies(new).is_some_and(|b| a == b))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_bodies_may_change_even_when_imports_share_a_function_line() {
        assert!(only_function_bodies_changed(
            b"package p; func F() int {return 1}",
            b"package p; func F() int {return 2}"
        ));
        assert!(!only_function_bodies_changed(
            b"package p; import _ \"fmt\"; func F() {}",
            b"package p; import _ \"os\"; func F() {}"
        ));
        assert!(!only_function_bodies_changed(
            b"package p; var X=1; func F() {}",
            b"package p; var X=2; func F() {}"
        ));
        assert!(!only_function_bodies_changed(
            b"package p; func F() {}",
            b"package p; func F(x int) {}"
        ));
        assert!(!only_function_bodies_changed(
            b"package p; func F() {}",
            b"package p; func G() {}"
        ));
        assert!(!only_function_bodies_changed(
            b"package p; func F() {}",
            b"package p; func F() {"
        ));
    }
}
