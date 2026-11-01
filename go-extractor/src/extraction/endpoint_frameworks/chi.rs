use crate::extraction::shared::{
    evaluate_expression_node, is_http_method_selector, parse_http_method_value, selector_name,
};

use super::shared::{
    ROUTER_PARAMETER_TYPES, call_arguments, is_known_router_receiver, is_router_parameter,
    looks_like_http_path, normalized_handler, router_variable,
};
use super::{EndpointIdentificationStrategy, EndpointMatch, ExtractParams};

pub(super) struct ChiStrategy;

impl EndpointIdentificationStrategy for ChiStrategy {
    fn identify(&self, params: &ExtractParams<'_>) -> Option<EndpointMatch> {
        let function_node = params.node.child_by_field_name("function")?;
        if !is_known_router_receiver(
            function_node,
            params.code,
            params.assignments,
            params.imports,
        ) && !is_router_parameter(function_node, params, ROUTER_PARAMETER_TYPES)
        {
            return None;
        }
        let selector = selector_name(function_node, params.code)?;
        let arguments = call_arguments(params.node);

        match selector.as_str() {
            s if matches!(
                s,
                "Get" | "Post" | "Put" | "Delete" | "Patch" | "Options" | "Head"
            ) || is_http_method_selector(s) =>
            {
                identify_two_arg_route(function_node, s, &arguments, params)
            }
            "Method" | "MethodFunc" => {
                if arguments.len() < 3 {
                    return None;
                }
                let path = evaluate_expression_node(arguments[1], params.code, params.globals);
                if !looks_like_http_path(&path) {
                    return None;
                }
                Some(EndpointMatch {
                    method: parse_http_method_value(&evaluate_expression_node(
                        arguments[0],
                        params.code,
                        params.globals,
                    ))?,
                    path,
                    handler: normalized_handler(arguments[2], params.code),
                    router: router_variable(function_node, params),
                })
            }
            _ => None,
        }
    }
}

fn identify_two_arg_route(
    function_node: tree_sitter::Node<'_>,
    selector: &str,
    arguments: &[tree_sitter::Node<'_>],
    params: &ExtractParams<'_>,
) -> Option<EndpointMatch> {
    if arguments.len() < 2 {
        return None;
    }
    let path = evaluate_expression_node(arguments[0], params.code, params.globals);
    if !looks_like_http_path(&path) {
        return None;
    }
    Some(EndpointMatch {
        method: parse_http_method_value(selector)?,
        path,
        handler: normalized_handler(arguments[1], params.code),
        router: router_variable(function_node, params),
    })
}
