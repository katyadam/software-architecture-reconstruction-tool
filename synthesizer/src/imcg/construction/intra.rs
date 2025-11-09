use std::collections::HashMap;

use models::{CallStatement, Import};

use crate::{
    errors::builder::BuilderError,
    imcg::model::{Call, ServiceCallable},
};

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
        callables: Vec<ServiceCallable>,
        call_statements: Vec<CallStatement>,
        imports: Vec<Import>,
    ) -> Result<CallGraph, BuilderError> {
        let callables_map = Self::get_callables_map(&callables);

        let calls: Vec<Call> = call_statements
            .iter()
            .filter_map(|stmt| {
                let function_hash = stmt.enclosing_function_hash.as_ref()?;
                let source = callables_map.get(function_hash)?;
                let target_id = Self::find_target_id(&stmt, &callables)?;
                Some(Call::new(
                    source.callable.signature.clone(),
                    target_id,
                    None,
                ))
            })
            .collect();

        Ok(CallGraph::new(vec![], calls))
    }

    fn get_callables_map(callables: &[ServiceCallable]) -> HashMap<String, &ServiceCallable> {
        let mut map = HashMap::with_capacity(callables.len());
        for c in callables {
            map.insert(c.callable.signature.clone(), c);
        }
        map
    }

    fn find_target_id(
        call_statement: &CallStatement,
        callables: &[ServiceCallable],
    ) -> Option<String> {
        // Searching for target that lays in Class, due to invoked_on is not None - Some(class_name)
        if call_statement.invoked_on.is_some() {
            return Self::invoked_on_lookup(call_statement, callables);
        }
        // Searching for target using target callable name and callable name matching
        Self::exhaustive_lookup(call_statement, callables)
    }

    fn exhaustive_lookup(
        call_statement: &CallStatement,
        callables: &[ServiceCallable],
    ) -> Option<String> {
        callables.iter().for_each(|callable| {});
        None
    }

    fn invoked_on_lookup(
        call_statement: &CallStatement,
        callables: &[ServiceCallable],
    ) -> Option<String> {
        todo!()
    }
}
