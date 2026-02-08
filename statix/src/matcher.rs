use std::collections::HashMap;

use crate::{ast::CallableAst, symbolic::VarType};

pub trait CallableMatcher {
    fn find_closest_callable(
        &self,
        callables: &HashMap<String, CallableAst>,
        name: &str,
        params: &[VarType],
    ) -> Option<String>;

    fn clone_box(&self) -> Box<dyn CallableMatcher>;
}

impl Clone for Box<dyn CallableMatcher> {
    fn clone(&self) -> Box<dyn CallableMatcher> {
        self.clone_box()
    }
}
