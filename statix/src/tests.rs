use tree_sitter::Node;

use crate::{
    parser::parse_method,
    util::{get_tree, load_file},
};

#[test]
fn testing() {
    let file_name =
        "/home/adamkattan/muni/VOYANTCLAIR/statix/src/examples/RestCallAfterMethodInvocation.java"
            .to_string();
    let code = load_file(&file_name).unwrap();
    let tree = get_tree(&code);
    let root_node = tree.root_node();
    let method_nodes = find_method_nodes(root_node);

    for method_node in method_nodes {
        let method_ast = parse_method(method_node, &code).unwrap();
        println!("Parsed method: {:#?}", method_ast);
    }
}

fn find_method_nodes(root: Node) -> Vec<Node> {
    let mut methods = Vec::new();
    let mut cursor = root.walk();

    for child in root.named_children(&mut cursor) {
        if child.kind() == "method_declaration" {
            methods.push(child);
        }
        methods.extend(find_method_nodes(child));
    }

    methods
}
