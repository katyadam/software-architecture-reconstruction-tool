use models::{CallStatement, RestCall};

use crate::extraction::restcalls::{
    evaluation::strategy::EvaluationStrategy, identification::strategy::IdentificationStrategy,
};

pub trait Selector {
    fn identification_strategy(&self) -> &dyn IdentificationStrategy;
    fn evaluation_strategy(&self) -> &dyn EvaluationStrategy;

    fn identify(&self, call: &CallStatement, file_path: &str) -> Option<RestCall> {
        self.identification_strategy()
            .identify_restcall(call, file_path)
    }

    fn evaluate(&self, restcall: &RestCall) -> Option<RestCall> {
        self.evaluation_strategy().evaluate_restcall(&restcall)
    }

    fn select_restcall_statements(
        &self,
        call_statements: &[CallStatement],
        file_path: &str,
    ) -> Vec<RestCall> {
        call_statements
            .iter()
            .filter_map(|call| {
                if let Some(restcall) = self.identify(call, file_path) {
                    self.evaluate(&restcall)
                } else {
                    None
                }
            })
            .collect()
    }
}
