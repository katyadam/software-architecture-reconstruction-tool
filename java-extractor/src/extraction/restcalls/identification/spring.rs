use std::str::FromStr;

use models::{Argument, CallStatement, HttpMethod, RestCall};

use crate::extraction::restcalls::identification::{HTTP_METHODS, strategy::Strategy};

pub struct SpringStrategy {}

impl SpringStrategy {
    pub fn new() -> Self {
        Self {}
    }
}

impl SpringStrategy {
    fn identify_http_method(&self, call_args: &[Argument]) -> Option<HttpMethod> {
        let arg = call_args.get(1)?;

        HTTP_METHODS
            .iter()
            .find(|m| {
                arg.value.to_ascii_lowercase().contains(*m)
                    || arg.datatype.to_ascii_lowercase().contains(*m)
            })
            .and_then(|m| HttpMethod::from_str(m).ok())
    }

    fn identify_target_uri(&self, call_args: &[Argument]) -> Option<String> {
        call_args.first().map(|uri| uri.value.clone())
    }
}

impl Strategy for SpringStrategy {
    fn identify_restcall(&self, call: &CallStatement, file_path: &str) -> Option<RestCall> {
        let http_method = self.identify_http_method(&call.arguments)?;
        let target_uri = self.identify_target_uri(&call.arguments)?;

        let invoked_on = call.invoked_on.clone()?;
        if invoked_on != "RestTemplate" {
            return None;
        }

        Some(RestCall {
            function_name: call.enclosing_function_name.clone().unwrap_or_default(),
            function_hash: call.enclosing_function_hash.clone().unwrap_or_default(),
            call_arguments: call.arguments.clone(),
            http_method,
            target_uri,
            file_path: file_path.to_string(),
        })
    }
}
