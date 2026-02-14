use crate::extraction::restcalls::{
    evaluation::method_call::MethodCallEvaluationStrategy,
    identification::method_call::MethodCallIdentificationStrategy, selection::selector::Selector,
};

pub struct MethodCallSelector {
    identification_strategy: MethodCallIdentificationStrategy,
    evaluation_strategy: MethodCallEvaluationStrategy,
}

impl MethodCallSelector {
    pub fn new(
        identification_strategy: MethodCallIdentificationStrategy,
        evaluation_strategy: MethodCallEvaluationStrategy,
    ) -> Self {
        Self {
            identification_strategy,
            evaluation_strategy,
        }
    }
}

impl Selector for MethodCallSelector {
    fn identification_strategy(
        &self,
    ) -> &dyn crate::extraction::restcalls::identification::strategy::IdentificationStrategy {
        &self.identification_strategy
    }

    fn evaluation_strategy(
        &self,
    ) -> &dyn crate::extraction::restcalls::evaluation::strategy::EvaluationStrategy {
        &self.evaluation_strategy
    }
}
