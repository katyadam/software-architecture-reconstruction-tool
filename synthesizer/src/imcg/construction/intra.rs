use std::collections::HashMap;

use log::debug;
use models::{CallStatement, Namespace};

use crate::{
    errors::builder::BuilderError,
    imcg::{
        construction::matching::get_score,
        model::{Call, ServiceCallable},
    },
};

#[derive(Debug)]
pub struct CallGraph {
    pub callables: Vec<ServiceCallable>,
    pub calls: Vec<Call>,
}

impl CallGraph {
    pub fn new(callables: Vec<ServiceCallable>, calls: Vec<Call>) -> Self {
        Self { callables, calls }
    }
}

pub struct CallGraphBuilderImpl {}

impl CallGraphBuilderImpl {
    pub fn new() -> Self {
        Self {}
    }

    pub fn build(
        self,
        callables: Vec<ServiceCallable>,
        callables_map: &HashMap<String, ServiceCallable>,
        call_statements: Vec<CallStatement>,
    ) -> Result<CallGraph, BuilderError> {
        let calls: Vec<Call> = call_statements
            .iter()
            .filter_map(|stmt| {
                let function_hash = stmt.enclosing_function_hash.as_ref()?;
                let source = callables_map.get(function_hash);
                match source {
                    Some(src) => {
                        let target_id = Self::find_target_id(&stmt, &callables, &src.service_name)?;
                        Some(Call::new(src.callable.signature.clone(), target_id, None))
                    }
                    None => None,
                }
            })
            .collect();

        Ok(CallGraph::new(callables, calls))
    }

    fn find_target_id(
        call_statement: &CallStatement,
        callables: &[ServiceCallable],
        source_service_name: &str,
    ) -> Option<String> {
        // Searching for target that lays in Class, due to invoked_on is not None - Some(class_name)
        if call_statement.invoked_on.is_some() {
            return Self::invoked_on_lookup(call_statement, callables, source_service_name);
        }
        // Searching for target using target callable name and callable name matching
        Self::exhaustive_lookup(call_statement, callables, source_service_name)
    }

    fn exhaustive_lookup(
        call_statement: &CallStatement,
        callables: &[ServiceCallable],
        source_service_name: &str,
    ) -> Option<String> {
        callables
            .iter()
            .filter(|callable| callable.service_name == source_service_name)
            .map(|callable| {
                let score = get_score(callable, call_statement);
                (score, callable) // pair score + callable
            })
            .max_by_key(|(score, _)| *score) // pick highest score
            .and_then(|(score, callable)| {
                if score > 0 {
                    Some(callable.callable.signature.clone())
                } else {
                    None
                }
            })
    }

    fn invoked_on_lookup(
        call_statement: &CallStatement,
        callables: &[ServiceCallable],
        source_service_name: &str,
    ) -> Option<String> {
        let invoked_on = call_statement.invoked_on.as_deref()?;

        callables
            .iter()
            .filter(|callable| callable.service_name == source_service_name)
            .find(|callable| {
                if let Namespace::Class(ref class_name) = callable.callable.namespace {
                    class_name == invoked_on
                } else {
                    false
                }
            })
            .map(|c| c.callable.signature.to_string())
    }
}
