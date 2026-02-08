use std::str::FromStr;

use models::{Argument, HttpMethod, RestCall};

use crate::extraction::restcalls::identification::{
    HTTP_METHODS, strategy::IdentificationStrategy,
};

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
        call_args.first().map(|uri| clean_python_string(&uri.value))
    }
}

impl IdentificationStrategy for MethodCallIdentificationStrategy {
    // To recognize REST call we just need the function_name to end with any HTTP method
    fn identify_restcall(
        &self,
        call_statement: &models::CallStatement,
        file_path: &str,
    ) -> Option<models::RestCall> {
        let http_method = self.identify_http_method(&call_statement.function_name)?;
        let target_uri = self.identify_target_uri(&call_statement.arguments)?;
        Some(RestCall {
            function_name: call_statement
                .enclosing_function_name
                .clone()
                .unwrap_or_default(),
            function_hash: call_statement
                .enclosing_function_hash
                .clone()
                .unwrap_or_default(),
            call_arguments: call_statement.arguments.clone(),
            http_method,
            target_uri,
            file_path: file_path.to_string(),
        })
    }
}

fn clean_python_string(s: &str) -> String {
    let quote_start = s.find(|c| c == '"' || c == '\'').unwrap_or(0);
    let content = &s[quote_start..];

    let cleaned = strip_quotes(content);

    cleaned
}

fn strip_quotes(s: &str) -> String {
    if s.starts_with("\"\"\"") || s.starts_with("'''") {
        s[3..s.len() - 3].to_string()
    } else {
        s.trim_matches(|c| c == '"' || c == '\'').to_string()
    }
}
