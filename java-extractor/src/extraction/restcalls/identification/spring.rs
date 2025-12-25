use std::str::FromStr;

use models::{Argument, HttpMethod};

use crate::extraction::restcalls::identification::{
    HTTP_METHODS, strategy::Strategy, utils::http_method_in_call_arguments,
};

pub struct SpringStrategy<'a> {
    callable_name: &'a str,
    call_args: &'a [Argument],
}

impl<'a> SpringStrategy<'a> {
    pub fn new(callable_name: &'a str, call_args: &'a [Argument]) -> Self {
        Self {
            callable_name,
            call_args,
        }
    }
}

impl<'a> Strategy<'a> for SpringStrategy<'_> {
    fn identify_http_method(&self) -> Option<HttpMethod> {
        if let Some(found) = HTTP_METHODS.iter().find(|m| {
            self.callable_name.to_ascii_lowercase().contains(*m)
                || http_method_in_call_arguments(self.call_args, m)
        }) {
            return HttpMethod::from_str(found).ok();
        }

        None
    }

    fn identify_target_uri(&self) -> Option<String> {
        self.call_args.first().map(|uri| uri.value.clone())
    }
}
