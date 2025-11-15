use std::collections::HashMap;

use models::{CallStatement, Import, Namespace};

use crate::{
    errors::builder::BuilderError,
    imcg::{
        construction::matching::get_score,
        model::{Call, ServiceCallable},
    },
};

#[derive(Debug)]
pub struct CallGraph {
    callables: Vec<ServiceCallable>,
    calls: Vec<Call>,
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
                let source = callables_map.get(function_hash);
                match source {
                    Some(src) => {
                        let target_id = Self::find_target_id(&stmt, &callables)?;
                        Some(Call::new(src.callable.signature.clone(), target_id, None))
                    }
                    None => None,
                }
            })
            .collect();

        Ok(CallGraph::new(callables, calls))
    }

    fn get_callables_map(callables: &[ServiceCallable]) -> HashMap<String, &ServiceCallable> {
        let mut map = HashMap::with_capacity(callables.len());
        for c in callables {
            map.insert(c.callable.hash.clone(), c);
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
        callables
            .iter()
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
    ) -> Option<String> {
        let invoked_on = call_statement.invoked_on.as_deref()?;

        callables
            .iter()
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

mod tests {
    use models::{Argument, CallStatement, Callable, Namespace, Parameter};

    use crate::imcg::{construction::intra::CallGraphBuilderImpl, model::ServiceCallable};

    #[test]
    fn testing() {
        let callables = get_callables();
        let call_statements = get_call_statements();
        let res = CallGraphBuilderImpl::build(callables, call_statements, vec![]).unwrap();
        println!("{res:?}");
    }

    fn get_call_statements() -> Vec<CallStatement> {
        vec![CallStatement {
            function_name: "A".to_string(),
            arguments: vec![
                Argument {
                    assigned_variable: "".to_string(),
                    value: "a".to_string(),
                    datatype: "int".to_string(),
                },
                Argument {
                    assigned_variable: "".to_string(),
                    value: "c".to_string(),
                    datatype: "any".to_string(),
                },
            ],
            enclosing_function_name: Some("B(a: int)".to_string()),
            enclosing_class_name: None,
            enclosing_function_hash: Some(
                "f9200cee7b04503e164b685447c9b5b6d8b6c46d64b4a1349e039db057512c55".to_string(),
            ),
            is_self_invoke: false,
            invoked_on: None,
        }]
    }

    fn get_callables() -> Vec<ServiceCallable> {
        vec![
            ServiceCallable {
                callable: Callable {
                    name: "A(x: int, y: int)".to_string(),
                    signature: "module:./examples/python/callgraph/simple.py/A(x: int, y: int)"
                        .to_string(),
                    namespace: Namespace::Module(
                        "./examples/python/callgraph/simple.py".to_string(),
                    ),
                    parameters: vec![
                        Parameter {
                            name: "x".to_string(),
                            datatype: Some("int".to_string()),
                            initial_value: None,
                        },
                        Parameter {
                            name: "y".to_string(),
                            datatype: Some("int".to_string()),
                            initial_value: None,
                        },
                    ],
                    return_type: None,
                    is_async: false,
                    is_constructor: false,
                    hash: "d74ff0ee8da3b9806b18c877dbf29bbde50b5bd8e4dad7a3a725000feb82e8f1"
                        .to_string(),
                    file_path: "./examples/python/callgraph/simple.py".to_string(),
                },
                service_name: "Test".to_string(),
            },
            ServiceCallable {
                callable: Callable {
                    name: "B(a: int)".to_string(),
                    signature: "module:./examples/python/callgraph/simple.py/B(a: int)".to_string(),
                    namespace: Namespace::Module(
                        "./examples/python/callgraph/simple.py".to_string(),
                    ),
                    parameters: vec![],
                    return_type: None,
                    is_async: false,
                    is_constructor: false,
                    hash: "f9200cee7b04503e164b685447c9b5b6d8b6c46d64b4a1349e039db057512c55"
                        .to_string(),
                    file_path: "./examples/python/callgraph/simple.py".to_string(),
                },
                service_name: "Test".to_string(),
            },
        ]
    }
}
