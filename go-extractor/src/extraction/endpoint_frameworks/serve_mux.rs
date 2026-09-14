use crate::extraction::shared::{evaluate_expression_node, selector_name, split_method_and_path};

use super::shared::{
    call_arguments, infer_method_from_handler, is_serve_mux_receiver, looks_like_http_path,
    normalized_handler,
};
use super::{EndpointIdentificationStrategy, EndpointMatch, ExtractParams};

pub(super) struct ServeMuxHandleStrategy;
pub(super) struct ServeMuxHandleFuncStrategy;

impl EndpointIdentificationStrategy for ServeMuxHandleStrategy {
    fn identify(&self, params: &ExtractParams<'_>) -> Option<EndpointMatch> {
        let function_node = params.node.child_by_field_name("function")?;
        if selector_name(function_node, params.code)? != "Handle" {
            return None;
        }
        if !is_serve_mux_receiver(
            function_node,
            params.code,
            params.assignments,
            params.imports,
        ) {
            return None;
        }

        let arguments = call_arguments(params.node);
        if arguments.len() < 2 {
            return None;
        }

        let path = evaluate_expression_node(arguments[0], params.code, params.globals);
        if !looks_like_http_path(&path) {
            return None;
        }

        let handler = normalized_handler(arguments[1], params.code);
        Some(EndpointMatch {
            method: infer_method_from_handler(&handler),
            path,
            handler,
        })
    }
}

impl EndpointIdentificationStrategy for ServeMuxHandleFuncStrategy {
    fn identify(&self, params: &ExtractParams<'_>) -> Option<EndpointMatch> {
        let function_node = params.node.child_by_field_name("function")?;
        if selector_name(function_node, params.code)? != "HandleFunc" {
            return None;
        }
        if !is_serve_mux_receiver(
            function_node,
            params.code,
            params.assignments,
            params.imports,
        ) {
            return None;
        }

        let arguments = call_arguments(params.node);
        if arguments.len() < 2 {
            return None;
        }

        let resolved = evaluate_expression_node(arguments[0], params.code, params.globals);
        let (method, path) = split_method_and_path(&resolved)?;
        Some(EndpointMatch {
            method,
            path,
            handler: normalized_handler(arguments[1], params.code),
        })
    }
}
