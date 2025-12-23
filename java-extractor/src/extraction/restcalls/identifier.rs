use std::str::FromStr;

use models::{Argument, HttpMethod};

const HTTP_METHODS: &[&str] = &["get", "post", "delete", "put", "patch"];

// TODO: Should look at the invoked object (if there is one) and based on its identify if it can call a restcall
// TODO: Maybe also look at the args?
// TODO: For that there is a need to do Type Inference for the invoked object as well as Data Flow Analysis
// to get the right uri that is being called
pub fn is_restcall(
    invoked_on: Option<String>,
    callable_name: String,
    call_args: &[Argument],
) -> bool {
    if callable_name == "exchange" {
        return true;
    }

    false
}

pub fn identify_http_method(callable_name: &str, call_args: &[Argument]) -> Option<HttpMethod> {
    if let Some(found) = HTTP_METHODS.iter().find(|m| {
        callable_name.to_ascii_lowercase().contains(*m)
            || http_method_in_call_arguments(call_args, m)
    }) {
        return HttpMethod::from_str(*found).ok();
    }

    None
}

fn http_method_in_call_arguments(call_args: &[Argument], http_method: &str) -> bool {
    call_args.iter().any(|arg| {
        arg.value.to_ascii_lowercase().contains(http_method)
            || arg.datatype.to_ascii_lowercase().contains(http_method)
    })
}
