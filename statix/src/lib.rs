use std::collections::HashMap;

use tree_sitter::Tree;

use crate::{
    ast::CallableAst,
    java::parser::{find_method_nodes, parse_method},
    python::parser::{find_function_nodes, parse_python_function},
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

pub fn parse_java(tree: &Tree, code: &str) -> HashMap<String, CallableAst> {
    let root_node = tree.root_node();
    let method_nodes = find_method_nodes(root_node);
    let mut methods_map: HashMap<String, CallableAst> = HashMap::new();
    for method_node in method_nodes {
        let method_ast = parse_method(method_node, code).unwrap();
        methods_map.insert(method_ast.header.clone(), method_ast.clone());
    }

    methods_map
}

pub fn parse_python(tree: &Tree, code: &str) -> HashMap<String, CallableAst> {
    let root_node = tree.root_node();
    let function_nodes = find_function_nodes(root_node);
    let mut functions_map: HashMap<String, CallableAst> = HashMap::new();
    for function_node in function_nodes {
        let function_ast = parse_python_function(function_node, code).unwrap();
        functions_map.insert(function_ast.header.clone(), function_ast.clone());
    }

    functions_map
}

pub fn symbolic_evaluation(
    callables_map: &HashMap<String, CallableAst>,
    callable_signature: &str,
) -> Result<symbolic::AnalysisResult, error::EvalError> {
    SymbolicEvaluator::eval_callable(callable_signature, callables_map)
}
