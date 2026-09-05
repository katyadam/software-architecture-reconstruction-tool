use crate::extraction::shared::{
    evaluate_expression_node, node_text, selector_name, web_route_method,
};

use super::shared::{WEB_IMPORT_PREFIXES, call_arguments, normalized_handler};
use super::{EndpointIdentificationStrategy, EndpointMatch, ExtractParams};

pub(super) struct WebStrategy;

impl EndpointIdentificationStrategy for WebStrategy {
    fn identify(&self, params: &ExtractParams<'_>) -> Option<EndpointMatch> {
        let function_node = params.node.child_by_field_name("function")?;
        if !is_web_route_call(function_node, params.code, params.imports) {
            return None;
        }

        let selector = selector_name(function_node, params.code)?;
        let method = web_route_method(&selector)?;
        let arguments = call_arguments(params.node);
        if arguments.len() < 2 {
            return None;
        }

        Some(EndpointMatch {
            method,
            path: evaluate_expression_node(arguments[0], params.code, params.globals),
            handler: normalized_handler(arguments[1], params.code),
        })
    }
}

fn is_web_route_call(
    function_node: tree_sitter::Node,
    code: &str,
    imports: &[models::Import],
) -> bool {
    let Some(selector) = selector_name(function_node, code) else {
        return false;
    };
    if web_route_method(&selector).is_none() {
        return false;
    }
    let Some(receiver) = function_node.child_by_field_name("operand") else {
        return false;
    };
    let receiver = node_text(receiver, code).trim();
    imports.iter().any(|import| {
        import.module_alias == receiver
            && WEB_IMPORT_PREFIXES
                .iter()
                .any(|prefix| import.orig_module.starts_with(prefix))
    })
}
