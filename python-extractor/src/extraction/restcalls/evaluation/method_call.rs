use std::collections::HashMap;

use models::{ParsedCallable, RestCall};
use statix::{
    python::matcher::{PythonCallableMatcher, python_convert_full_header_to_mangled_name},
    symbolic_evaluation,
};

use crate::extraction::restcalls::evaluation::{
    strategy::EvaluationStrategy, uri_generator::generate_target_uris,
};

pub struct MethodCallEvaluationStrategy {
    function_asts: HashMap<String, ParsedCallable>,
    enums_map: HashMap<String, Vec<String>>,
}

impl MethodCallEvaluationStrategy {
    pub fn new(
        function_asts: HashMap<String, ParsedCallable>,
        enums_map: HashMap<String, Vec<String>>,
    ) -> Self {
        Self {
            function_asts,
            enums_map,
        }
    }
}

impl EvaluationStrategy for MethodCallEvaluationStrategy {
    fn evaluate_restcall(
        &self,
        restcall: &models::RestCall,
    ) -> Result<Vec<models::RestCall>, statix::error::EvalError> {
        let mangled_header = python_convert_full_header_to_mangled_name(&restcall.function_name);
        let analysis_result = symbolic_evaluation(
            &self.function_asts,
            &mangled_header,
            Box::new(PythonCallableMatcher::new()),
        )?;
        let mut evaluated_restcalls: Vec<RestCall> = Vec::new();
        let target_uris =
            generate_target_uris(&restcall.target_uri, &analysis_result, &self.enums_map);
        for uri in target_uris {
            evaluated_restcalls.push(restcall.clone_from_target_uri(&uri));
        }

        Ok(evaluated_restcalls)
    }
}
