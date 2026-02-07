use std::collections::HashMap;

use models::RestCall;
use statix::{
    ast::CallableAst, callable_match::convert_full_header_to_mangled_name, error::EvalError,
    symbolic::SymbolicEvaluator,
};

use crate::extraction::restcalls::evaluation::{
    strategy::EvaluationStrategy, uri_generator::generate_target_uris,
};

pub struct SpringEvaluationStrategy {
    method_asts: HashMap<String, CallableAst>,
}

impl SpringEvaluationStrategy {
    pub fn new(method_asts: HashMap<String, CallableAst>) -> Self {
        Self { method_asts }
    }
}

impl EvaluationStrategy for SpringEvaluationStrategy {
    fn evaluate_restcall(&self, restcall: &RestCall) -> Result<Vec<RestCall>, EvalError> {
        let mangled_header = convert_full_header_to_mangled_name(&restcall.function_name);
        let analysis_result = SymbolicEvaluator::eval_callable(&mangled_header, &self.method_asts)?;
        let mut evaluated_restcalls: Vec<RestCall> = Vec::new();
        let target_uris = generate_target_uris(&restcall.target_uri, &analysis_result);
        for uri in target_uris {
            evaluated_restcalls.push(restcall.clone_from_target_uri(&uri));
        }

        Ok(evaluated_restcalls)
    }
}
