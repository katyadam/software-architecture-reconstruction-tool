use std::collections::HashMap;

use tree_sitter::Node;

use crate::{
    ast::MethodAst,
    parser::{find_method_nodes, parse_method},
    symbolic::eval_method,
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
    let mut methods_map: HashMap<String, MethodAst> = HashMap::new();
    for method_node in method_nodes {
        let method_ast = parse_method(method_node, &code).unwrap();
        methods_map.insert(method_ast.name.clone(), method_ast.clone());
    }
    let res_env = eval_method("drawbackMoney", &methods_map);
    println!("{res_env:#?}");
}
