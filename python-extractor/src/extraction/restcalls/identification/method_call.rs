use std::str::FromStr;

use models::{Argument, HttpMethod, RestCall, source_code::SourceSpan};
use tree_sitter::Node;

use crate::extraction::{
    calls::PythonCallStatement,
    restcalls::identification::{HTTP_METHODS, strategy::IdentificationStrategy},
};

#[derive(Default)]
pub struct MethodCallIdentificationStrategy {}

impl MethodCallIdentificationStrategy {
    pub fn new() -> Self {
        Self {}
    }

    fn identify_http_method(&self, function_name: &str) -> Option<HttpMethod> {
        if let Some(last) = function_name.split(".").last() {
            HTTP_METHODS
                .iter()
                .find(|m| last.to_ascii_lowercase() == **m)
                .and_then(|m| HttpMethod::from_str(m).ok())
        } else {
            None
        }
    }

    fn identify_target_uri(&self, call_args: &[Argument]) -> Option<String> {
        let raw = &call_args.first()?.value;
        let uri = statix::strings::clean_python_string(raw);
        let is_string_literal = raw.contains('"') || raw.contains('\'');
        // String literals with no `/` are not URIs (e.g. dict key lookups like `.get("key")`).
        // Bare variable names (no quotes) are passed through — evaluation resolves them later.
        if is_string_literal && !uri.contains('/') {
            return None;
        }
        Some(uri)
    }

    // FastAPI uses @app.http_method to denote endpoint, therefore we want to omit that here
    fn is_part_of_decorator(&self, call_statement_node: Node) -> bool {
        if let Some(parent_node) = call_statement_node.parent() {
            return parent_node.kind() == "decorator";
        }
        false
    }
}

impl IdentificationStrategy for MethodCallIdentificationStrategy {
    // To recognize REST call we just need the function_name to end with any HTTP method
    fn identify_restcall(
        &self,
        call: &PythonCallStatement,
        file_path: &str,
    ) -> Option<models::RestCall> {
        let http_method = self.identify_http_method(&call.call_statement.function_name)?;
        let target_uri = self.identify_target_uri(&call.call_statement.arguments)?;
        if self.is_part_of_decorator(call.node) {
            return None;
        }
        if call.call_statement.enclosing_function_name.is_none()
            && call.call_statement.enclosing_class_name.is_none()
        {
            return None;
        }
        Some(RestCall {
            function_name: call
                .call_statement
                .enclosing_function_name
                .clone()
                .unwrap_or_default(),
            function_hash: call
                .call_statement
                .enclosing_function_hash
                .clone()
                .unwrap_or_default(),
            call_arguments: call.call_statement.arguments.clone(),
            http_method,
            target_uri,
            file_path: file_path.to_string(),
            source_span: SourceSpan::default(),
        })
    }
}
