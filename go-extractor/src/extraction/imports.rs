use models::Import;
use statix::strings::strip_quotes;
use tree_sitter::Node;

use super::shared::{node_text, walk_named};

pub(super) fn collect_imports(root: Node, code: &str) -> Vec<Import> {
    let mut imports = Vec::new();
    walk_named(root, &mut |node| {
        if node.kind() != "import_spec" {
            return;
        }

        let Some(path_node) = node.child_by_field_name("path").or_else(|| {
            node.named_children(&mut node.walk())
                .find(|child| child.kind() == "interpreted_string_literal")
        }) else {
            return;
        };
        let path = strip_quotes(node_text(path_node, code)).to_string();
        let default_alias = path.rsplit('/').next().unwrap_or(&path);
        let alias = node
            .child_by_field_name("name")
            .map(|name| node_text(name, code))
            .filter(|name| !matches!(*name, "_" | "."))
            .unwrap_or(default_alias);

        imports.push(Import::from_parts(&path, "", alias, "", alias));
    });
    imports
}
