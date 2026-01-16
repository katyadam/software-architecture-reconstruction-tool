use models::RestCall;

use crate::extraction::restcalls::evaluation::strategy::EvaluationStrategy;

pub struct SpringEvaluationStrategy {}

impl SpringEvaluationStrategy {
    pub fn new() -> Self {
        Self {}
    }
}

impl EvaluationStrategy for SpringEvaluationStrategy {
    fn evaluate_restcall(&self, restcall: &RestCall) -> Option<RestCall> {
        todo!()
    }
}
