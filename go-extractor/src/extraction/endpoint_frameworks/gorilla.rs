use crate::extraction::shared::{evaluate_expression_node, parse_http_method_value, selector_name};

use super::shared::{call_arguments, function_node, normalized_handler};
use super::{EndpointIdentificationStrategy, EndpointMatch, ExtractParams};

pub(super) struct GorillaStrategy;

impl EndpointIdentificationStrategy for GorillaStrategy {
    fn identify(&self, params: &ExtractParams<'_>) -> Option<EndpointMatch> {
        let function_node = function_node(params.node)?;
        if selector_name(function_node, params.code)? != "Methods" {
            return None;
        }

        let selector_operand = function_node.child_by_field_name("operand")?;
        if selector_operand.kind() != "call_expression" {
            return None;
        }

        let inner_function = selector_operand.child_by_field_name("function")?;
        if selector_name(inner_function, params.code)? != "HandleFunc" {
            return None;
        }

        let inner_args = call_arguments(selector_operand);
        let method_args = call_arguments(params.node);
        if inner_args.len() < 2 || method_args.is_empty() {
            return None;
        }

        Some(EndpointMatch {
            method: parse_http_method_value(&evaluate_expression_node(
                method_args[0],
                params.code,
                params.globals,
            ))?,
            path: evaluate_expression_node(inner_args[0], params.code, params.globals),
            handler: normalized_handler(inner_args[1], params.code),
        })
    }
}
