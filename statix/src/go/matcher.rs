use std::collections::HashMap;

use models::ParsedCallable;

use crate::{matcher::find_closest_callable_impl, symbolic::VarType};

#[derive(Clone, Default)]
pub struct GoCallableMatcher {}

impl GoCallableMatcher {
    pub fn new() -> Self {
        Self {}
    }
}

impl crate::matcher::CallableMatcher for GoCallableMatcher {
    fn find_closest_callable(
        &self,
        callables: &HashMap<String, ParsedCallable>,
        name: &str,
        params: &[VarType],
    ) -> Option<String> {
        find_closest_callable_impl(callables, name, params)
    }

    fn clone_box(&self) -> Box<dyn crate::matcher::CallableMatcher> {
        Box::new(self.clone())
    }
}

pub fn go_convert_full_header_to_mangled_name(header: &str) -> String {
    header.to_string()
}
