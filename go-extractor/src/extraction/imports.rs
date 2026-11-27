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
        // Go major-version suffix (`chi/v5`, v2+) is not the package name.
        // shortcut: a package actually named `v2` (k8s.io/api/autoscaling/v2) gets its parent's name.
        let mut segments = path.rsplit('/');
        let last = segments.next().unwrap_or(&path);
        let is_major_version = last
            .strip_prefix('v')
            .and_then(|n| n.parse::<u32>().ok())
            .is_some_and(|n| n >= 2);
        let default_alias = match segments.next() {
            Some(package) if is_major_version => package,
            _ => last,
        };
        let alias = node
            .child_by_field_name("name")
            .map(|name| node_text(name, code))
            .filter(|name| !matches!(*name, "_" | "."))
            .unwrap_or(default_alias);

        imports.push(Import::from_parts(&path, "", alias, "", alias));
    });
    imports
}
