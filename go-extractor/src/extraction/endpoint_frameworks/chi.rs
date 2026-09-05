use crate::extraction::shared::{
    evaluate_expression_node, is_http_method_selector, parse_http_method_value, selector_name,
};

use super::shared::{
    call_arguments, is_known_router_receiver, join_route_prefix, looks_like_http_path,
    normalized_handler, router_prefix,
};
use super::{EndpointIdentificationStrategy, EndpointMatch, ExtractParams};

pub(super) struct ChiStrategy;

impl EndpointIdentificationStrategy for ChiStrategy {
    fn identify(&self, params: &ExtractParams<'_>) -> Option<EndpointMatch> {
        let function_node = params.node.child_by_field_name("function")?;
        let prefix = router_prefix(
            function_node,
            params.code,
            params.assignments,
            params.imports,
        )?;
        if prefix.is_empty()
            && !is_known_router_receiver(
                function_node,
                params.code,
                params.assignments,
                params.imports,
            )
        {
            return None;
        }
        let selector = selector_name(function_node, params.code)?;
        let arguments = call_arguments(params.node);

        match selector.as_str() {
            "Get" | "Post" | "Put" | "Delete" | "Patch" | "Options" | "Head" => {
                identify_two_arg_route(&prefix, selector.as_str(), &arguments, params)
            }
            _ if is_http_method_selector(selector.as_str()) => {
                identify_two_arg_route(&prefix, selector.as_str(), &arguments, params)
            }
            "Method" | "MethodFunc" => {
                if arguments.len() < 3 {
                    return None;
                }
                let path = join_route_prefix(
                    &prefix,
                    &evaluate_expression_node(arguments[1], params.code, params.globals),
                );
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
                })
            }
            _ => None,
        }
    }
}

fn identify_two_arg_route(
    prefix: &str,
    selector: &str,
    arguments: &[tree_sitter::Node<'_>],
    params: &ExtractParams<'_>,
) -> Option<EndpointMatch> {
    if arguments.len() < 2 {
        return None;
    }
    let path = join_route_prefix(
        prefix,
        &evaluate_expression_node(arguments[0], params.code, params.globals),
    );
    if !looks_like_http_path(&path) {
        return None;
    }
    Some(EndpointMatch {
        method: parse_http_method_value(selector)?,
        path,
        handler: normalized_handler(arguments[1], params.code),
    })
}
