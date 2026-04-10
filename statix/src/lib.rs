use std::collections::HashMap;

use models::{Callable, Namespace, ParsedCallable, ir::ast::CallableAst};
use tree_sitter::{Node, Tree};

use crate::{
    error::ParseError,
    java::parser::{find_method_nodes, parse_callable_parameters, parse_method},
    matcher::CallableMatcher,
    python::{
        matcher::python_convert_full_header_to_mangled_name,
        parser::{
            find_function_nodes, parse_callable_parameters as parse_python_callable_params,
            parse_python_function,
        },
    },
    symbolic::{AnalysisContext, SymbolicEvaluator},
};

pub mod class_hierarchy;
pub mod error;
pub mod import_graph;
pub mod java;
pub mod matcher;
pub mod python;
pub mod strings;
pub mod symbolic;
pub mod util;
pub mod visitor;

/// Parse all methods and return a `ParsedCallable` map keyed by the mangled header
/// (e.g. `"String getUrl()"`, `"boolean send(String,int)"`).
/// This is the form expected by `symbolic_evaluation`.
pub fn parse_java(tree: &Tree, code: &str) -> HashMap<String, ParsedCallable> {
    use java::matcher::java_convert_full_header_to_mangled_name;

    let root_node = tree.root_node();
    let method_nodes = find_method_nodes(root_node);
    let mut map = HashMap::new();

    for method_node in method_nodes {
        let return_type = method_node
            .child_by_field_name("type")
            .and_then(|n| n.utf8_text(code.as_bytes()).ok())
            .unwrap_or("")
            .to_string();
        let name = method_node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(code.as_bytes()).ok())
            .unwrap_or("")
            .to_string();
        let params_text = method_node
            .child_by_field_name("parameters")
            .and_then(|n| n.utf8_text(code.as_bytes()).ok())
            .unwrap_or("()")
            .to_string();

        let full_header = format!("{} {}{}", return_type, name, params_text);
        let mangled_key = java_convert_full_header_to_mangled_name(&full_header);

        let parameters = method_node
            .child_by_field_name("parameters")
            .map(|n| parse_callable_parameters(n, code))
            .unwrap_or_default();

        let Ok(ast) = parse_method(method_node, code) else {
            continue;
        };

        let callable = Callable {
            name: full_header.clone(),
            signature: full_header,
            namespace: Namespace::default(),
            parameters,
            return_type: Some(return_type),
            is_async: false,
            is_constructor: false,
            hash: String::new(),
            file_path: String::new(),
        };

        map.insert(
            mangled_key,
            ParsedCallable {
                metadata: callable,
                ast,
            },
        );
    }

    map
}

pub fn parse_java_method(method_node: &Node, code: &str) -> Result<CallableAst, ParseError> {
    parse_method(*method_node, code)
}

/// Parse all functions and return a `ParsedCallable` map keyed by the mangled header
/// (e.g. `"str get_url()"`, `"str build(Any,str,int)"`).
pub fn parse_python(tree: &Tree, code: &str) -> HashMap<String, ParsedCallable> {
    let root_node = tree.root_node();
    let function_nodes = find_function_nodes(root_node);
    let mut map = HashMap::new();

    for function_node in function_nodes {
        let name = function_node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(code.as_bytes()).ok())
            .unwrap_or("")
            .to_string();
        let params_text = function_node
            .child_by_field_name("parameters")
            .and_then(|n| n.utf8_text(code.as_bytes()).ok())
            .unwrap_or("()")
            .to_string();
        let return_type_text = function_node
            .child_by_field_name("return_type")
            .and_then(|n| n.utf8_text(code.as_bytes()).ok())
            .unwrap_or("Any")
            .to_string();

        // python_convert_full_header_to_mangled_name expects "name(params) -> return_type"
        let full_header = format!("{}{} -> {}", name, params_text, return_type_text);
        let mangled_key = python_convert_full_header_to_mangled_name(&full_header);

        let parameters = function_node
            .child_by_field_name("parameters")
            .map(|n| parse_python_callable_params(n, code))
            .unwrap_or_default();

        let Ok(ast) = parse_python_function(function_node, code) else {
            continue;
        };

        let callable = Callable {
            name: full_header.clone(),
            signature: full_header,
            namespace: Namespace::default(),
            parameters,
            return_type: Some(return_type_text),
            is_async: false,
            is_constructor: false,
            hash: String::new(),
            file_path: String::new(),
        };

        map.insert(
            mangled_key,
            ParsedCallable {
                metadata: callable,
                ast,
            },
        );
    }

    map
}

pub fn symbolic_evaluation(
    callables_map: &HashMap<String, ParsedCallable>,
    callable_signature: &str,
    matcher: Box<dyn CallableMatcher>,
) -> Result<symbolic::AnalysisResult, error::EvalError> {
    let ctx = AnalysisContext::new(callables_map, matcher.into());
    SymbolicEvaluator::eval_callable(callable_signature, &ctx)
}

/// Like [`symbolic_evaluation`] but seeds the evaluation environment with `initial_env`
/// before processing the callable's parameters. Use to define cross-file constants in the
/// symbolic evaluation environment.
pub fn symbolic_evaluation_with_env(
    callables_map: &HashMap<String, ParsedCallable>,
    callable_signature: &str,
    matcher: Box<dyn CallableMatcher>,
    initial_env: HashMap<String, (Option<String>, models::ir::ast::Expr)>,
) -> Result<symbolic::AnalysisResult, error::EvalError> {
    let ctx = AnalysisContext::new(callables_map, matcher.into());
    SymbolicEvaluator::eval_callable_with_env(callable_signature, &ctx, initial_env)
}
