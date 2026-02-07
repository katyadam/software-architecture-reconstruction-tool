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
        call_args.first().map(|uri| uri.value.clone())
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
