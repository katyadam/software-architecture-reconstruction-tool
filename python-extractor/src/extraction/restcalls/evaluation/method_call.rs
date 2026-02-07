use std::collections::HashMap;

use models::RestCall;
use statix::{
    ast::CallableAst, callable_match::convert_full_header_to_mangled_name,
    symbolic::SymbolicEvaluator,
};

use crate::extraction::restcalls::evaluation::{
    strategy::EvaluationStrategy, uri_generator::generate_target_uris,
};

pub struct MethodCallEvaluationStrategy {
    function_asts: HashMap<String, CallableAst>,
}

impl MethodCallEvaluationStrategy {
    pub fn new(function_asts: HashMap<String, CallableAst>) -> Self {
        Self { function_asts }
    }
}

impl EvaluationStrategy for MethodCallEvaluationStrategy {
    fn evaluate_restcall(
        &self,
        restcall: &models::RestCall,
    ) -> Result<Vec<models::RestCall>, statix::error::EvalError> {
        let mangled_header = convert_full_header_to_mangled_name(&restcall.function_name);
        let analysis_result =
            SymbolicEvaluator::eval_callable(&mangled_header, &self.function_asts)?;
        let mut evaluated_restcalls: Vec<RestCall> = Vec::new();
        let target_uris = generate_target_uris(&restcall.target_uri, &analysis_result);
        for uri in target_uris {
            evaluated_restcalls.push(restcall.clone_from_target_uri(&uri));
        }

        Ok(evaluated_restcalls)
    }
}
