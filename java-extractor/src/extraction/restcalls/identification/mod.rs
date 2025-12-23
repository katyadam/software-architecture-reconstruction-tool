use models::Argument;

use crate::extraction::restcalls::identification::{spring::SpringStrategy, strategy::Strategy};

pub mod spring;
pub mod strategy;
mod utils;
const HTTP_METHODS: &[&str] = &["get", "post", "delete", "put", "patch"];

// TODO: Should look at the invoked object (if there is one) and based on its identify if it can call a restcall
// TODO: Maybe also look at the args?
// TODO: For that there is a need to do Type Inference for the invoked object as well as Data Flow Analysis
// to get the right uri that is being called
pub fn get_identification_strategy<'a>(
    _invoked_on: Option<String>,
    callable_name: &'a str,
    call_args: &'a [Argument],
) -> Option<impl Strategy<'a>> {
    if callable_name == "exchange" {
        return Some(SpringStrategy::new(callable_name, call_args));
    }
    None
}
