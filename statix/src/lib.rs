use std::collections::HashMap;

use tree_sitter::Tree;

use crate::{
    ast::CallableAst,
    java::parser::{find_method_nodes, parse_method},
    symbolic::SymbolicEvaluator,
};

pub mod ast;
pub mod callable_match;
pub mod error;
pub mod java;
pub mod python;
pub mod symbolic;
pub mod util;
pub mod visitor;

pub fn parse_methods(tree: &Tree, code: &str) -> HashMap<String, CallableAst> {
    let root_node = tree.root_node();
    let method_nodes = find_method_nodes(root_node);
    let mut methods_map: HashMap<String, CallableAst> = HashMap::new();
    for method_node in method_nodes {
        let method_ast = parse_method(method_node, code).unwrap();
        methods_map.insert(method_ast.header.clone(), method_ast.clone());
    }

    methods_map
}

pub fn symbolic_evaluation(
    methods_map: &HashMap<String, CallableAst>,
    callable_signature: &str,
) -> Result<symbolic::AnalysisResult, error::EvalError> {
    SymbolicEvaluator::eval_method(callable_signature, methods_map)
}
